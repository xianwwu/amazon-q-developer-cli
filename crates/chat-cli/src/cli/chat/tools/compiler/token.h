#ifndef TOKEN_H
#define TOKEN_H

// Token types for SimpleLang
typedef enum {
    // Literals
    TOKEN_NUMBER,
    TOKEN_IDENTIFIER,
    TOKEN_STRING,
    
    // Keywords
    TOKEN_IF,
    TOKEN_ELSE,
    TOKEN_WHILE,
    TOKEN_PRINT,
    TOKEN_READ,
    TOKEN_INT,
    TOKEN_STRING_TYPE,
    
    // Operators
    TOKEN_PLUS,
    TOKEN_MINUS,
    TOKEN_MULTIPLY,
    TOKEN_DIVIDE,
    TOKEN_ASSIGN,
    TOKEN_EQUAL,
    TOKEN_NOT_EQUAL,
    TOKEN_LESS,
    TOKEN_GREATER,
    TOKEN_LESS_EQUAL,
    TOKEN_GREATER_EQUAL,
    
    // Delimiters
    TOKEN_SEMICOLON,
    TOKEN_COMMA,
    TOKEN_LEFT_PAREN,
    TOKEN_RIGHT_PAREN,
    TOKEN_LEFT_BRACE,
    TOKEN_RIGHT_BRACE,
    
    // Special
    TOKEN_NEWLINE,
    TOKEN_EOF,
    TOKEN_ERROR
} TokenType;

typedef struct {
    TokenType type;
    char* lexeme;
    int line;
    int column;
    union {
        int int_value;
        char* string_value;
    } value;
} Token;

// Function declarations
Token* create_token(TokenType type, char* lexeme, int line, int column);
void free_token(Token* token);
const char* token_type_to_string(TokenType type);

#endif
