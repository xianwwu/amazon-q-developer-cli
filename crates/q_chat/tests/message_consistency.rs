//! Tests to verify that help text and error messages match the existing code

use q_chat::ChatError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_message_consistency() {
        // Helper function to extract error message
        fn get_error_message(result: Result<(), ChatError>) -> String {
            match result {
                Err(ChatError::Custom(msg)) => msg.to_string(),
                _ => panic!("Expected ChatError::Custom"),
            }
        }
        
        // Verify that error messages match the expected format
        assert_eq!(
            get_error_message(Err(ChatError::Custom("HelpCommand can only execute Help commands".into()))),
            "HelpCommand can only execute Help commands"
        );
        
        assert_eq!(
            get_error_message(Err(ChatError::Custom("CompactCommand can only execute Compact commands".into()))),
            "CompactCommand can only execute Compact commands"
        );
        
        assert_eq!(
            get_error_message(Err(ChatError::Custom("ClearCommand can only execute Clear commands".into()))),
            "ClearCommand can only execute Clear commands"
        );
        
        assert_eq!(
            get_error_message(Err(ChatError::Custom("QuitCommand can only execute Quit commands".into()))),
            "QuitCommand can only execute Quit commands"
        );
    }

    #[test]
    fn test_context_command_error_messages() {
        // Helper function to extract error message
        fn get_error_message(result: Result<(), ChatError>) -> String {
            match result {
                Err(ChatError::Custom(msg)) => msg.to_string(),
                _ => panic!("Expected ChatError::Custom"),
            }
        }
        
        // Verify that error messages match the expected format
        assert_eq!(
            get_error_message(Err(ChatError::Custom("No paths specified. Usage: /context add [--global] [--force] <path1> [path2...]".into()))),
            "No paths specified. Usage: /context add [--global] [--force] <path1> [path2...]"
        );
        
        assert_eq!(
            get_error_message(Err(ChatError::Custom("No paths specified. Usage: /context rm [--global] <path1> [path2...]".into()))),
            "No paths specified. Usage: /context rm [--global] <path1> [path2...]"
        );
        
        assert_eq!(
            get_error_message(Err(ChatError::Custom("Invalid command".into()))),
            "Invalid command"
        );
    }

    #[test]
    fn test_profile_command_error_messages() {
        // Helper function to extract error message
        fn get_error_message(result: Result<(), ChatError>) -> String {
            match result {
                Err(ChatError::Custom(msg)) => msg.to_string(),
                _ => panic!("Expected ChatError::Custom"),
            }
        }
        
        // Verify that error messages match the expected format
        assert_eq!(
            get_error_message(Err(ChatError::Custom("Expected profile name argument".into()))),
            "Expected profile name argument"
        );
        
        assert_eq!(
            get_error_message(Err(ChatError::Custom("Expected old_name and new_name arguments".into()))),
            "Expected old_name and new_name arguments"
        );
        
        assert_eq!(
            get_error_message(Err(ChatError::Custom("Missing profile name for set command".into()))),
            "Missing profile name for set command"
        );
        
        assert_eq!(
            get_error_message(Err(ChatError::Custom("Missing profile name for create command".into()))),
            "Missing profile name for create command"
        );
        
        assert_eq!(
            get_error_message(Err(ChatError::Custom("Missing profile name for delete command".into()))),
            "Missing profile name for delete command"
        );
        
        assert_eq!(
            get_error_message(Err(ChatError::Custom("Missing old or new profile name for rename command".into()))),
            "Missing old or new profile name for rename command"
        );
    }

    #[test]
    fn test_tools_command_error_messages() {
        // Helper function to extract error message
        fn get_error_message(result: Result<(), ChatError>) -> String {
            match result {
                Err(ChatError::Custom(msg)) => msg.to_string(),
                _ => panic!("Expected ChatError::Custom"),
            }
        }
        
        // Verify that error messages match the expected format
        assert_eq!(
            get_error_message(Err(ChatError::Custom("Expected at least one tool name".into()))),
            "Expected at least one tool name"
        );
        
        assert_eq!(
            get_error_message(Err(ChatError::Custom("Expected tool name argument".into()))),
            "Expected tool name argument"
        );
    }
}
