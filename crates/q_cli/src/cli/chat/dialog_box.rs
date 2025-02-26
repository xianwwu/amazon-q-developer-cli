use std::io::Write;

use crossterm::{queue, style};
use unicode_width::UnicodeWidthStr;

const TOP_LEFT_CORNER: &str = "┌";
const TOP_RIGHT_CORNER: &str = "┐";
const BOT_LEFT_CORNER: &str = "└";
const BOT_RIGHT_CORNER: &str = "┘";
const HOR_LINE: &str = "─";
const VER_LINE: &str = "│";

pub struct DialogBox<'a> {
    title: &'a str,
    text_to_display: &'a str,
    padding: usize,
}

impl<'a> DialogBox<'a> {
    pub fn new(title: &'a str, text_to_display: &'a str, padding: usize) -> Self {
        Self {
            title,
            text_to_display,
            padding,
        }
    }

    pub fn queue_boxed_output<W: Write>(&self, output: &mut W) -> Result<(), std::io::Error> {
        let longest_line_width = self.text_to_display.lines().fold(0_usize, |acc, line| {
            let grapheme_width = UnicodeWidthStr::width_cjk(line);
            if acc > grapheme_width { acc } else { grapheme_width }
        });
        let box_width = longest_line_width
            + self.padding * 2 // assuming a space has a grapheme width of 1
            + UnicodeWidthStr::width_cjk(TOP_LEFT_CORNER)
            + UnicodeWidthStr::width_cjk(TOP_RIGHT_CORNER);
        let mut boxed_content = TOP_LEFT_CORNER.to_string();
        boxed_content.push_str({
            let hor_line_width =
                box_width - UnicodeWidthStr::width_cjk(TOP_LEFT_CORNER) - UnicodeWidthStr::width_cjk(TOP_RIGHT_CORNER);
            &HOR_LINE.repeat(hor_line_width)
        });
        boxed_content.push_str(TOP_RIGHT_CORNER);
        boxed_content.push('\n');
        for line in self.text_to_display.lines() {
            let mut new_line = VER_LINE.to_string();
            new_line.push_str(&" ".repeat(self.padding));
            new_line.push_str(line.trim());
            new_line.push_str(&" ".repeat(self.padding));
            new_line.push_str(VER_LINE);
            new_line.push('\n');
            boxed_content.push_str(&new_line);
        }
        boxed_content.push_str(BOT_LEFT_CORNER);
        boxed_content.push_str({
            let hor_line_width =
                box_width - UnicodeWidthStr::width_cjk(BOT_LEFT_CORNER) - UnicodeWidthStr::width_cjk(BOT_RIGHT_CORNER);
            &HOR_LINE.repeat(hor_line_width)
        });
        boxed_content.push_str(BOT_RIGHT_CORNER);
        boxed_content.push('\n');

        queue!(output, style::Print(boxed_content))
    }
}
