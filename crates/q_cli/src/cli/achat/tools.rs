use std::borrow::Cow;
use std::collections::{
    HashMap,
    VecDeque,
};
use std::fs::Metadata;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use aws_sdk_bedrockruntime::types::{
    Tool as BedrockTool,
    ToolInputSchema as BedrockToolInputSchema,
    ToolResultContentBlock,
    ToolSpecification as BedrockToolSpecification,
};
use aws_smithy_types::{
    Document,
    Number as SmithyNumber,
};
use eyre::{
    Result,
    bail,
};
use fig_os_shim::{
    Context,
    ContextArcProvider,
};
use nix::unistd::{
    geteuid,
    getuid,
};
use serde::Deserialize;
use thiserror::Error;
use tracing::{
    debug,
    error,
    info,
    warn,
};


pub const FILE_READ: &str = r#"
{
  "name": "file_read",
  "description": "A tool for viewing files and directories.\n* If `path` is a file, this tool displays the result of applying `cat -n`.\n* If `path` is a directory, this tool lists files and directories\n",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": {
        "description": "Absolute path to file or directory, e.g. `/repo/file.py` or `/repo`.",
        "type": "string"
      },
      "read_range": {
        "description": "Optional parameter when reading either files or directories.\n* When `path` is a file, if none is given, the full file is shown. If provided, the file will be shown in the indicated line number range, e.g. [11, 12] will show lines 11 and 12. Indexing at 1 to start. Setting `[start_line, -1]` shows all lines from `start_line` to the end of the file.\n* When `path` is a directory, if none is given, the results of `ls -l` are given. If provided, the current directory and indicated number of subdirectories will be shown, e.g. [2] will show the current directory and directories two levels deep.",
        "items": {
          "type": "integer"
        },
        "type": "array"
      }
    },
    "required": [
      "path"
    ]
  }
}
"#;

pub const FILE_WRITE: &str = r#"
{
  "name": "file_write",
  "description": "Custom editing tool for creating and editing files\n * The `create` command cannot be used if the specified `path` already exists as a file\n * If a `command` generates a long output, it will be truncated and marked with `<response clipped>` \n Notes for using the `str_replace` command:\n * The `old_str` parameter should match EXACTLY one or more consecutive lines from the original file. Be mindful of whitespaces!\n * If the `old_str` parameter is not unique in the file, the replacement will not be performed. Make sure to include enough context in `old_str` to make it unique\n * The `new_str` parameter should contain the edited lines that should replace the `old_str`",
  "input_schema": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "enum": [
          "create",
          "str_replace",
          "insert"
        ],
        "description": "The commands to run. Allowed options are: `create`, `str_replace`, `insert`."
      },
      "file_text": {
        "description": "Required parameter of `create` command, with the content of the file to be created.",
        "type": "string"
      },
      "insert_line": {
        "description": "Required parameter of `insert` command. The `new_str` will be inserted AFTER the line `insert_line` of `path`.",
        "type": "integer"
      },
      "new_str": {
        "description": "Required parameter of `str_replace` command containing the new string. Required parameter of `insert` command containing the string to insert.",
        "type": "string"
      },
      "old_str": {
        "description": "Required parameter of `str_replace` command containing the string in `path` to replace.",
        "type": "string"
      },
      "path": {
        "description": "Absolute path to file or directory, e.g. `/repo/file.py` or `/repo`.",
        "type": "string"
      }
    },
    "required": [
      "command",
      "path"
    ]
  }
}

"#;

pub const EXECUTE_BASH: &str = r#"
{
  "name": "execute_bash",
  "description": "Execute the specified bash command",
  "input_schema": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "description": "Bash command to execute"
      }
    },
    "required": [
      "command"
    ]
  }
}
"#;

pub fn load_tools() -> HashMap<String, ToolSpec> {
    let file_read = file_read();
    HashMap::from([(file_read.name.clone(), file_read)])
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: InputSchema,
}

impl From<ToolSpec> for BedrockTool {
    fn from(value: ToolSpec) -> Self {
        BedrockTool::ToolSpec(value.into())
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<ToolSpec> for BedrockToolSpecification {
    fn from(value: ToolSpec) -> Self {
        BedrockToolSpecification::builder()
            .name(value.name)
            .description(value.description)
            .input_schema(value.input_schema.into())
            .build()
            .unwrap()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InputSchema(serde_json::Value);

impl From<InputSchema> for BedrockToolInputSchema {
    fn from(value: InputSchema) -> Self {
        BedrockToolInputSchema::Json(serde_value_to_document(value.0))
    }
}

#[derive(Debug, Default)]
pub struct InvokeOutput {
    pub output: OutputKind,
}

impl InvokeOutput {
    fn text(&self) -> Option<&str> {
        match &self.output {
            OutputKind::Text(text) => Some(text),
            _ => None,
        }
    }
}

impl From<InvokeOutput> for ToolResultContentBlock {
    fn from(value: InvokeOutput) -> Self {
        match value.output {
            OutputKind::Text(text) => ToolResultContentBlock::Text(text),
        }
    }
}

#[non_exhaustive]
#[derive(Debug)]
enum OutputKind {
    Text(String),
}

impl Default for OutputKind {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

#[derive(Debug, Error)]
pub enum ToolError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    SystemTime(#[from] std::time::SystemTimeError),
    #[error("{0}")]
    InvalidToolUse(Cow<'static, str>),
    #[error("{0}")]
    Custom(Cow<'static, str>),
}

#[async_trait]
pub trait Tool: std::fmt::Debug {
    async fn invoke(&self) -> Result<InvokeOutput, ToolError>;
    fn requires_consent(&self) -> bool {
        false
    }
}

pub fn new_tool<C: ContextArcProvider>(ctx: C, name: &str, value: serde_json::Value) -> Result<Box<dyn Tool + Sync>> {
    let tool = match name {
        "file_read" => Box::new(FileRead::from_value(ctx.context_arc(), value)?) as Box<dyn Tool + Sync>,
        "file_write" => Box::new(FileWrite::from_value(ctx.context_arc(), value)?) as Box<dyn Tool + Sync>,
        "execute_bash" => Box::new(ExecuteBash::from_value(ctx.context_arc(), value)?) as Box<dyn Tool + Sync>,
        custom_name => bail!("custom tools are not supported: model request tool {}", custom_name),
    };
    Ok(tool)
}

#[derive(Debug)]
pub struct FileRead {
    ctx: Arc<Context>,
    pub args: FileReadArgs,
}

impl FileRead {
    pub fn from_value(ctx: Arc<Context>, args: serde_json::Value) -> Result<Self, ToolError> {
        Ok(Self {
            ctx,
            args: serde_json::from_value(args)?,
        })
    }

    pub fn read_range(&self) -> Result<Option<(i32, Option<i32>)>, ToolError> {
        match &self.args.read_range {
            Some(range) => match (range.get(0), range.get(1)) {
                (Some(depth), None) => Ok(Some((*depth, None))),
                (Some(start), Some(end)) => Ok(Some((*start, Some(*end)))),
                other => Err(ToolError::Custom(format!("Invalid read range: {:?}", other).into())),
            },
            None => Ok(None),
        }
    }
}

#[async_trait]
impl Tool for FileRead {
    async fn invoke(&self) -> Result<InvokeOutput, ToolError> {
        // Required for testing scenarios: since the path is passed directly as a command argument,
        // we need to pass it through the Context first.
        let path = self.ctx.fs().chroot_path_str(&self.args.path);
        let is_file = self.ctx.fs().symlink_metadata(&self.args.path).await?.is_file();

        if is_file {
            // TODO: file size limit?
            let file = self.ctx.fs().read_to_string(&path).await?;

            if let Some((start, Some(end))) = self.read_range()? {
                let line_count = file.lines().count();

                // Convert negative 1-based indices to positive 0-based indices.
                let convert_index = |i: i32| -> usize {
                    if i <= 0 {
                        (line_count as i32 + i) as usize
                    } else {
                        i as usize - 1
                    }
                };
                let (start, end) = (convert_index(start), convert_index(end));
                if start > end {
                    return Ok(InvokeOutput {
                        output: OutputKind::Text(String::new()),
                    });
                }

                // The range should be inclusive on both ends.
                let f = file
                    .lines()
                    .skip(start)
                    .take(end - start + 1)
                    .collect::<Vec<_>>()
                    .join("\n");
                return Ok(InvokeOutput {
                    output: OutputKind::Text(f),
                });
            }
            return Ok(InvokeOutput {
                output: OutputKind::Text(file),
            });
        } else {
            let max_depth = self.read_range()?.map_or(0, |(d, _)| d);
            let mut result = Vec::new();
            let mut dir_queue = VecDeque::new();
            dir_queue.push_back((PathBuf::from(path), 0));
            while let Some((path, depth)) = dir_queue.pop_front() {
                if depth > max_depth {
                    break;
                }
                let mut read_dir = self.ctx.fs().read_dir(path).await?;
                while let Some(ent) = read_dir.next_entry().await? {
                    use std::os::unix::fs::MetadataExt;
                    let md = ent.metadata().await?;
                    let formatted_mode = format_mode(md.permissions().mode()).into_iter().collect::<String>();

                    let modified_timestamp = md.modified()?.duration_since(std::time::UNIX_EPOCH)?.as_secs();
                    let datetime = time::OffsetDateTime::from_unix_timestamp(modified_timestamp as i64).unwrap();
                    let formatted_date = datetime
                        .format(time::macros::format_description!(
                            "[month repr:short] [day] [hour]:[minute]"
                        ))
                        .unwrap();

                    // Mostly copying "The Long Format" from `man ls`.
                    // TODO: query user/group database to convert uid/gid to names?
                    result.push(format!(
                        "{}{} {} {} {} {} {} {}",
                        format_ftype(&md),
                        formatted_mode,
                        md.nlink(),
                        md.uid(),
                        md.gid(),
                        md.size(),
                        formatted_date,
                        ent.path().to_string_lossy()
                    ));
                    if md.is_dir() {
                        dir_queue.push_back((ent.path(), depth + 1));
                    }
                }
            }
            return Ok(InvokeOutput {
                output: OutputKind::Text(result.join("\n")),
            });
        }
    }
}

fn format_ftype(md: &Metadata) -> char {
    if md.is_symlink() {
        'l'
    } else if md.is_file() {
        '-'
    } else if md.is_dir() {
        'd'
    } else {
        warn!("unknown file metadata: {:?}", md);
        '-'
    }
}

/// Formats a permissions mode into the form used by `ls`, e.g. `0o644` to `rw-r--r--`
fn format_mode(mode: u32) -> [char; 9] {
    let mut mode = mode & 0o777;
    let mut res = ['-'; 9];
    fn octal_to_chars(val: u32) -> [char; 3] {
        match val {
            1 => ['-', '-', 'x'],
            2 => ['-', 'w', '-'],
            3 => ['-', 'w', 'x'],
            4 => ['r', '-', '-'],
            5 => ['r', '-', 'x'],
            6 => ['r', 'w', '-'],
            7 => ['r', 'w', 'x'],
            _ => ['-', '-', '-'],
        }
    }
    for c in res.rchunks_exact_mut(3) {
        c.copy_from_slice(&octal_to_chars(mode & 0o7));
        mode /= 0o10;
    }
    res
}

#[derive(Debug, Deserialize)]
pub struct FileReadArgs {
    pub path: String,
    pub read_range: Option<Vec<i32>>,
}

#[derive(Debug)]
pub struct FileWrite {
    ctx: Arc<Context>,
    pub args: FileWriteArgs,
}

impl FileWrite {
    pub fn from_value(ctx: Arc<Context>, args: serde_json::Value) -> Result<Self, ToolError> {
        Ok(Self {
            ctx,
            args: serde_json::from_value(args)?,
        })
    }
}

#[async_trait]
impl Tool for FileWrite {
    async fn invoke(&self) -> Result<InvokeOutput, ToolError> {
        // let fs = self.ctx.fs();
        // match self.args {
        //     FileWriteArgs::Create { path, file_text } => {
        //         let path = fs.chroot_path_str(path);
        //         tokio::fs::OpenOptions
        //
        //     },
        //     FileWriteArgs::StrReplace { path, old_str, new_str } => todo!(),
        //     FileWriteArgs::Insert {
        //         path,
        //         insert_line,
        //         new_str,
        //     } => todo!(),
        // }
        Ok(Default::default())
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command")]
pub enum FileWriteArgs {
    #[serde(rename = "create")]
    Create { path: String, file_text: String },
    #[serde(rename = "str_replace")]
    StrReplace {
        path: String,
        old_str: String,
        new_str: String,
    },
    #[serde(rename = "insert")]
    Insert {
        path: String,
        insert_line: usize,
        new_str: String,
    },
}

#[derive(Debug)]
pub struct ExecuteBash {
    ctx: Arc<Context>,
    pub args: ExecuteBashArgs,
}

impl ExecuteBash {
    pub fn from_value(ctx: Arc<Context>, args: serde_json::Value) -> Result<Self, ToolError> {
        Ok(Self {
            ctx,
            args: serde_json::from_value(args)?,
        })
    }
}

#[async_trait]
impl Tool for ExecuteBash {
    async fn invoke(&self) -> Result<InvokeOutput, ToolError> {
        Ok(Default::default())
    }
}

#[derive(Debug, Deserialize)]
pub struct ExecuteBashArgs {
    pub command: String,
}

/// Returns the "file_read" tool specification.
pub fn file_read() -> ToolSpec {
    serde_json::from_str(FILE_READ).expect("deserializing tool spec should succeed")
}

/// Returns the "file_write" tool specification.
pub fn file_write() -> ToolSpec {
    serde_json::from_str(FILE_WRITE).expect("deserializing tool spec should succeed")
}

/// Returns the "execute_bash" tool specification.
pub fn execute_bash() -> ToolSpec {
    serde_json::from_str(EXECUTE_BASH).expect("deserializing tool spec should succeed")
}

#[derive(Debug)]
pub struct Custom {}

#[async_trait]
impl Tool for Custom {
    async fn invoke(&self) -> Result<InvokeOutput, ToolError> {
        warn!("Not implemented");
        Ok(Default::default())
    }
}

pub fn serde_value_to_document(value: serde_json::Value) -> Document {
    match value {
        serde_json::Value::Null => Document::Null,
        serde_json::Value::Bool(bool) => Document::Bool(bool),
        serde_json::Value::Number(number) => {
            // todo
            Document::Number(SmithyNumber::Float(number.as_f64().unwrap()))
        },
        serde_json::Value::String(string) => Document::String(string),
        serde_json::Value::Array(vec) => {
            Document::Array(vec.clone().into_iter().map(serde_value_to_document).collect::<_>())
        },
        serde_json::Value::Object(map) => Document::Object(
            map.into_iter()
                .map(|(k, v)| (k, serde_value_to_document(v)))
                .collect::<_>(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FILE_CONTENTS: &str = "\
1: Hello world!
2: This is line 2
3: asdf
4: Hello world!
";

    const TEST_FILE_PATH: &str = "/test_file.txt";
    const TEST_HIDDEN_FILE_PATH: &str = "/aaaa2/.hidden";

    /// Sets up the following filesystem structure:
    /// ```text
    /// test_file.txt
    /// /home/testuser/
    /// /aaaa1/
    ///     /bbbb1/
    ///         /cccc1/
    /// /aaaa2/
    ///     .hidden
    /// ```
    async fn setup_test_directory() -> Arc<Context> {
        let ctx = Context::builder().with_test_home().await.unwrap().build_fake();
        let fs = ctx.fs();
        fs.write(TEST_FILE_PATH, TEST_FILE_CONTENTS).await.unwrap();
        fs.create_dir_all("/aaaa1/bbbb1/cccc1").await.unwrap();
        fs.create_dir_all("/aaaa2").await.unwrap();
        fs.write(TEST_HIDDEN_FILE_PATH, "this is a hidden file").await.unwrap();
        ctx
    }

    #[test]
    fn test_tool_spec_deser() {
        file_read();
        file_write();
        execute_bash();
    }

    #[test]
    fn test_file_read_creation() {
        let ctx = Context::new_fake();
        let v = serde_json::json!({ "path": "/test_file.txt", "read_range": vec![1, 2] });
        let fr = FileRead::from_value(Arc::clone(&ctx), v).unwrap();
        assert_eq!(fr.args.path, TEST_FILE_PATH);
        assert_eq!(fr.args.read_range.unwrap(), vec![1, 2]);

        let v = serde_json::json!({ "path": "/test_file.txt", "read_range": vec![-1] });
        let fr = FileRead::from_value(Arc::clone(&ctx), v).unwrap();
        assert_eq!(fr.args.path, TEST_FILE_PATH);
        assert_eq!(fr.args.read_range.unwrap(), vec![-1]);
    }

    #[tokio::test]
    async fn test_file_read_tool_for_files() {
        let ctx = setup_test_directory().await;
        let lines = TEST_FILE_CONTENTS.lines().collect::<Vec<_>>();

        macro_rules! assert_lines {
            ($range:expr, $expected:expr) => {
                let v = serde_json::json!({
                    "path": TEST_FILE_PATH,
                    "read_range": $range,
                });
                let output = FileRead::from_value(Arc::clone(&ctx), v).unwrap().invoke().await.unwrap();
                let text = output.text().unwrap();
                assert_eq!(text, $expected.join("\n"), "actual(left) does not equal expected(right) for range: {:?}", $range);
            }
        }
        assert_lines!((1, 2), lines[..=1]);
        assert_lines!((1, -1), lines[..]);
        assert_lines!((2, 1), [] as [&str; 0]);
        assert_lines!((-2, -1), lines[2..]);
    }

    #[test]
    fn test_format_mode() {
        macro_rules! assert_mode {
            ($actual:expr, $expected:expr) => {
                assert_eq!(format_mode($actual).iter().collect::<String>(), $expected);
            };
        }
        assert_mode!(0o000, "---------");
        assert_mode!(0o700, "rwx------");
        assert_mode!(0o744, "rwxr--r--");
        assert_mode!(0o641, "rw-r----x");
    }

    #[tokio::test]
    async fn test_file_read_tool_for_directories() {
        let ctx = setup_test_directory().await;

        // Testing without depth
        let v = serde_json::json!({
            "path": "/",
            "read_range": None::<()>,
        });
        let output = FileRead::from_value(Arc::clone(&ctx), v)
            .unwrap()
            .invoke()
            .await
            .unwrap();
        let lines = output.text().unwrap().lines().collect::<Vec<_>>();
        // println!("{}", output.text().unwrap());
        assert_eq!(lines.len(), 4);

        // Testing with depth level 1
        let v = serde_json::json!({
            "path": "/",
            "read_range": Some(vec![1]),
        });
        let output = FileRead::from_value(Arc::clone(&ctx), v)
            .unwrap()
            .invoke()
            .await
            .unwrap();
        let lines = output.text().unwrap().lines().collect::<Vec<_>>();
        // println!("{}", output.text().unwrap());
        assert_eq!(lines.len(), 7);
        assert!(
            !lines.iter().any(|l| l.contains("cccc1")),
            "directory at depth level 2 should not be included in output"
        );
    }

    #[test]
    fn test_file_write_deserialize() {
        let ctx = Context::new_fake();
        let path = "/my-file";
        let file_text = "hello world";

        // create
        let v = serde_json::json!({
            "path": path,
            "command": "create",
            "file_text": file_text
        });
        let fw = FileWrite::from_value(Arc::clone(&ctx), v).unwrap();
        assert!(matches!(fw.args, FileWriteArgs::Create { .. }));

        // str_replace
        let v = serde_json::json!({
            "path": path,
            "command": "str_replace",
            "old_str": "prev string",
            "new_str": "new string",
        });
        let fw = FileWrite::from_value(Arc::clone(&ctx), v).unwrap();
        assert!(matches!(fw.args, FileWriteArgs::StrReplace { .. }));

        // insert
        let v = serde_json::json!({
            "path": path,
            "command": "insert",
            "insert_line": 3,
            "new_str": "new string",
        });
        let fw = FileWrite::from_value(Arc::clone(&ctx), v).unwrap();
        assert!(matches!(fw.args, FileWriteArgs::Insert { .. }));
    }

    #[tokio::test]
    async fn test_file_write_tool_create() {
        let ctx = setup_test_directory().await;

        let file_text = "Hello, world!";
        let v = serde_json::json!({
            "path": "/my-file",
            "command": "create",
            "file_text": file_text
        });
        FileWrite::from_value(Arc::clone(&ctx), v)
            .unwrap()
            .invoke()
            .await
            .unwrap();
        assert_eq!(ctx.fs().read_to_string("/my-file").await.unwrap(), file_text);
    }

    #[tokio::test]
    async fn test_execute_bash_tool() {}
}
