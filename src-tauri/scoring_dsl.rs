// scoring_dsl.rs — Hand-written recursive descent parser for .scoring DSL files
//
// This file is `include!`'d by build.rs. It compiles a `.scoring` config file
// into Rust constants at build time. No external dependencies (only std).
//
// Pipeline: Lex -> Parse -> Validate -> Generate

use std::fmt;

// ============================================================================
// Tokens
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
enum Token {
    Identifier(String),
    Number(f64),
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,
    Colon,
    FatArrow,
    Comma,
    Hash,
    At,
    Underscore,
    Newline,
    /// A comment line starting with `# constraint:` — preserved for validation
    Constraint(String),
    Eof,
}

#[derive(Debug, Clone)]
struct SpannedToken {
    token: Token,
    line: usize,
}

// ============================================================================
// AST
// ============================================================================

#[derive(Debug, Clone)]
struct Param {
    name: String,
    value: f64,
    range: Option<(f64, f64)>,
    line: usize,
}

#[derive(Debug, Clone)]
enum BoundsEntry {
    Range { name: String, min: f64, max: f64, line: usize },
    Scalar { name: String, value: f64, range: Option<(f64, f64)>, line: usize },
}

#[derive(Debug, Clone)]
struct Constraint {
    fields: Vec<String>,
    expected_sum: f64,
    line: usize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum AstNode {
    Version(u32),
    Calibration { name: String, params: Vec<Param> },
    ConfirmationGate { entries: Vec<(u8, (f64, f64))>, lines: Vec<usize> },
    SignalThresholds { entries: Vec<Param> },
    FreshnessTiers { tiers: Vec<(f64, f64)>, default: f64, tier_lines: Vec<usize>, default_line: usize },
    Weights { name: String, params: Vec<Param>, constraints: Vec<Constraint> },
    Bounds { entries: Vec<BoundsEntry> },
    ConfidenceBonuses { entries: Vec<(u8, f64)>, lines: Vec<usize> },
    SpecificityWeights { entries: Vec<Param> },
    KeywordAceBoost { entries: Vec<Param> },
    Serendipity { entries: Vec<Param> },
    PriorityCaps { entries: Vec<Param> },
    QualityFloor { entries: Vec<Param> },
    SemanticBoost { entries: Vec<Param> },
    ConfidenceFloor { entries: Vec<Param> },
}

// ============================================================================
// Validation errors
// ============================================================================

#[derive(Debug)]
struct ValidationError {
    line: usize,
    message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

// ============================================================================
// Stage 1: Lexer
// ============================================================================

fn lex(input: &str) -> Result<Vec<SpannedToken>, Vec<String>> {
    let mut tokens: Vec<SpannedToken> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let lines: Vec<&str> = input.lines().collect();

    for (line_idx, raw_line) in lines.iter().enumerate() {
        let line_num = line_idx + 1;
        let line = *raw_line;

        // Trim leading whitespace for detection, but we parse the full line
        let trimmed = line.trim();

        // Skip fully blank lines
        if trimmed.is_empty() {
            tokens.push(SpannedToken { token: Token::Newline, line: line_num });
            continue;
        }

        // Check for constraint comments: `# constraint: ...`
        if trimmed.starts_with("# constraint:") {
            let constraint_text = trimmed.trim_start_matches("# constraint:").trim().to_string();
            tokens.push(SpannedToken { token: Token::Constraint(constraint_text), line: line_num });
            tokens.push(SpannedToken { token: Token::Newline, line: line_num });
            continue;
        }

        // Pure comment lines (not after a value) — skip entirely
        if trimmed.starts_with('#') {
            tokens.push(SpannedToken { token: Token::Newline, line: line_num });
            continue;
        }

        // Tokenize the line character by character
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            // Skip whitespace (but not newline — we handle newlines at line level)
            if ch == ' ' || ch == '\t' || ch == '\r' {
                i += 1;
                continue;
            }

            // Comment: # may be a range annotation `# [min, max]` or a plain comment
            if ch == '#' {
                // Look ahead to see if this is a range annotation: # [
                let rest: String = chars[i..].iter().collect();
                let rest_trimmed = rest.trim_start_matches('#').trim_start();
                if rest_trimmed.starts_with('[') {
                    // Parse range annotation: # [min, max]
                    tokens.push(SpannedToken { token: Token::Hash, line: line_num });
                    i += 1;
                    // Skip whitespace between # and [
                    while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    // Now parse [min, max]
                    if i < chars.len() && chars[i] == '[' {
                        tokens.push(SpannedToken { token: Token::LBracket, line: line_num });
                        i += 1;
                        // Parse min
                        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                            i += 1;
                        }
                        let num_start = i;
                        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '-') {
                            i += 1;
                        }
                        if i > num_start {
                            let num_str: String = chars[num_start..i].iter().collect();
                            match num_str.parse::<f64>() {
                                Ok(n) => tokens.push(SpannedToken { token: Token::Number(n), line: line_num }),
                                Err(_) => errors.push(format!("line {}: invalid number '{}'", line_num, num_str)),
                            }
                        }
                        // comma
                        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                            i += 1;
                        }
                        if i < chars.len() && chars[i] == ',' {
                            tokens.push(SpannedToken { token: Token::Comma, line: line_num });
                            i += 1;
                        }
                        // Parse max
                        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                            i += 1;
                        }
                        let num_start = i;
                        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '-') {
                            i += 1;
                        }
                        if i > num_start {
                            let num_str: String = chars[num_start..i].iter().collect();
                            match num_str.parse::<f64>() {
                                Ok(n) => tokens.push(SpannedToken { token: Token::Number(n), line: line_num }),
                                Err(_) => errors.push(format!("line {}: invalid number '{}'", line_num, num_str)),
                            }
                        }
                        // ]
                        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                            i += 1;
                        }
                        if i < chars.len() && chars[i] == ']' {
                            tokens.push(SpannedToken { token: Token::RBracket, line: line_num });
                            i += 1;
                        }
                    }
                } else if rest_trimmed.starts_with("constraint:") {
                    // Inline constraint comment (shouldn't normally appear mid-line, but handle it)
                    let constraint_text = rest_trimmed.trim_start_matches("constraint:").trim().to_string();
                    tokens.push(SpannedToken { token: Token::Constraint(constraint_text), line: line_num });
                    break; // rest of line consumed
                } else {
                    // Plain comment — skip rest of line
                    break;
                }
                continue;
            }

            // Single character tokens
            match ch {
                '{' => { tokens.push(SpannedToken { token: Token::LBrace, line: line_num }); i += 1; continue; }
                '}' => { tokens.push(SpannedToken { token: Token::RBrace, line: line_num }); i += 1; continue; }
                '[' => { tokens.push(SpannedToken { token: Token::LBracket, line: line_num }); i += 1; continue; }
                ']' => { tokens.push(SpannedToken { token: Token::RBracket, line: line_num }); i += 1; continue; }
                '(' => { tokens.push(SpannedToken { token: Token::LParen, line: line_num }); i += 1; continue; }
                ')' => { tokens.push(SpannedToken { token: Token::RParen, line: line_num }); i += 1; continue; }
                ':' => { tokens.push(SpannedToken { token: Token::Colon, line: line_num }); i += 1; continue; }
                ',' => { tokens.push(SpannedToken { token: Token::Comma, line: line_num }); i += 1; continue; }
                '@' => { tokens.push(SpannedToken { token: Token::At, line: line_num }); i += 1; continue; }
                _ => {}
            }

            // Fat arrow =>
            if ch == '=' && i + 1 < chars.len() && chars[i + 1] == '>' {
                tokens.push(SpannedToken { token: Token::FatArrow, line: line_num });
                i += 2;
                continue;
            }

            // Numbers (including negative)
            if ch.is_ascii_digit() || (ch == '-' && i + 1 < chars.len() && (chars[i + 1].is_ascii_digit() || chars[i + 1] == '.')) {
                let start = i;
                if ch == '-' {
                    i += 1;
                }
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let num_str: String = chars[start..i].iter().collect();
                match num_str.parse::<f64>() {
                    Ok(n) => tokens.push(SpannedToken { token: Token::Number(n), line: line_num }),
                    Err(_) => errors.push(format!("line {}: invalid number '{}'", line_num, num_str)),
                }
                continue;
            }

            // Underscore (standalone wildcard for freshness default)
            if ch == '_' && (i + 1 >= chars.len() || !chars[i + 1].is_alphanumeric()) {
                // Check if this is start of an identifier like `_name`
                // Standalone _ is a wildcard; _foo is an identifier
                let peek = if i + 1 < chars.len() { chars[i + 1] } else { ' ' };
                if peek.is_alphabetic() || peek == '_' {
                    // It's an identifier starting with _
                    let start = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }
                    let ident: String = chars[start..i].iter().collect();
                    tokens.push(SpannedToken { token: Token::Identifier(ident), line: line_num });
                } else {
                    tokens.push(SpannedToken { token: Token::Underscore, line: line_num });
                    i += 1;
                }
                continue;
            }

            // Identifiers: [a-zA-Z_][a-zA-Z0-9_]*
            if ch.is_alphabetic() || ch == '_' {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();
                tokens.push(SpannedToken { token: Token::Identifier(ident), line: line_num });
                continue;
            }

            // Unknown character
            errors.push(format!("line {}: unexpected character '{}'", line_num, ch));
            i += 1;
        }

        tokens.push(SpannedToken { token: Token::Newline, line: line_num });
    }

    tokens.push(SpannedToken { token: Token::Eof, line: lines.len() + 1 });

    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors)
    }
}

// ============================================================================
// Stage 2: Parser (recursive descent)
// ============================================================================

struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<SpannedToken>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        if self.pos < self.tokens.len() {
            &self.tokens[self.pos].token
        } else {
            &Token::Eof
        }
    }

    fn current_line(&self) -> usize {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos].line
        } else if !self.tokens.is_empty() {
            self.tokens.last().unwrap().line
        } else {
            0
        }
    }

    fn advance(&mut self) -> &SpannedToken {
        let t = &self.tokens[self.pos];
        self.pos += 1;
        t
    }

    fn skip_newlines(&mut self) {
        while self.pos < self.tokens.len() {
            match &self.tokens[self.pos].token {
                Token::Newline => { self.pos += 1; }
                _ => break,
            }
        }
    }

    fn expect_token(&mut self, expected: &Token) -> Result<usize, Vec<String>> {
        self.skip_newlines();
        let line = self.current_line();
        let actual = self.peek().clone();
        if std::mem::discriminant(&actual) == std::mem::discriminant(expected) {
            self.advance();
            Ok(line)
        } else {
            Err(vec![format!("line {}: expected {:?}, found {:?}", line, expected, actual)])
        }
    }

    fn expect_identifier(&mut self) -> Result<(String, usize), Vec<String>> {
        self.skip_newlines();
        let line = self.current_line();
        match self.peek().clone() {
            Token::Identifier(s) => {
                self.advance();
                Ok((s, line))
            }
            other => Err(vec![format!("line {}: expected identifier, found {:?}", line, other)]),
        }
    }

    fn expect_number(&mut self) -> Result<(f64, usize), Vec<String>> {
        self.skip_newlines();
        let line = self.current_line();
        match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                Ok((n, line))
            }
            other => Err(vec![format!("line {}: expected number, found {:?}", line, other)]),
        }
    }

    fn expect_u8(&mut self) -> Result<(u8, usize), Vec<String>> {
        let (n, line) = self.expect_number()?;
        if n < 0.0 || n > 255.0 || n.fract() != 0.0 {
            return Err(vec![format!("line {}: expected integer 0-255, got {}", line, n)]);
        }
        Ok((n as u8, line))
    }

    /// Try to parse an optional range annotation: # [min, max]
    fn try_parse_range(&mut self) -> Option<(f64, f64)> {
        // Range annotations appear as: Hash LBracket Number Comma Number RBracket
        let saved_pos = self.pos;
        if let Token::Hash = self.peek() {
            self.advance();
            if let Token::LBracket = self.peek() {
                self.advance();
                if let Token::Number(min) = self.peek().clone() {
                    self.advance();
                    if let Token::Comma = self.peek() {
                        self.advance();
                        if let Token::Number(max) = self.peek().clone() {
                            self.advance();
                            if let Token::RBracket = self.peek() {
                                self.advance();
                                return Some((min, max));
                            }
                        }
                    }
                }
            }
        }
        // Backtrack on failure
        self.pos = saved_pos;
        None
    }

    /// Parse a named param: `name: value  # [min, max]`
    fn parse_param(&mut self) -> Result<Param, Vec<String>> {
        let (name, line) = self.expect_identifier()?;
        self.expect_token(&Token::Colon)?;
        let (value, _) = self.expect_number()?;
        let range = self.try_parse_range();
        Ok(Param { name, value, range, line })
    }

    /// Parse the full file
    fn parse(&mut self) -> Result<Vec<AstNode>, Vec<String>> {
        let mut nodes = Vec::new();
        let mut errors = Vec::new();

        loop {
            self.skip_newlines();

            match self.peek().clone() {
                Token::Eof => break,

                Token::At => {
                    // @version N
                    match self.parse_version() {
                        Ok(node) => nodes.push(node),
                        Err(mut e) => errors.append(&mut e),
                    }
                }

                Token::Identifier(ref ident) => {
                    let ident = ident.clone();
                    match ident.as_str() {
                        "calibration" => match self.parse_calibration() {
                            Ok(node) => nodes.push(node),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "confirmation_gate" => match self.parse_confirmation_gate() {
                            Ok(node) => nodes.push(node),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "signal_thresholds" => match self.parse_named_params_section("signal_thresholds") {
                            Ok(params) => nodes.push(AstNode::SignalThresholds { entries: params }),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "freshness_tiers" => match self.parse_freshness_tiers() {
                            Ok(node) => nodes.push(node),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "weights" => match self.parse_weights() {
                            Ok(node) => nodes.push(node),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "bounds" => match self.parse_bounds() {
                            Ok(node) => nodes.push(node),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "confidence_bonuses" => match self.parse_confidence_bonuses() {
                            Ok(node) => nodes.push(node),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "specificity_weights" => match self.parse_named_params_section("specificity_weights") {
                            Ok(params) => nodes.push(AstNode::SpecificityWeights { entries: params }),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "keyword_ace_boost" => match self.parse_named_params_section("keyword_ace_boost") {
                            Ok(params) => nodes.push(AstNode::KeywordAceBoost { entries: params }),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "serendipity" => match self.parse_named_params_section("serendipity") {
                            Ok(params) => nodes.push(AstNode::Serendipity { entries: params }),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "priority_caps" => match self.parse_named_params_section("priority_caps") {
                            Ok(params) => nodes.push(AstNode::PriorityCaps { entries: params }),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "quality_floor" => match self.parse_named_params_section("quality_floor") {
                            Ok(params) => nodes.push(AstNode::QualityFloor { entries: params }),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "semantic_boost" => match self.parse_semantic_boost() {
                            Ok(node) => nodes.push(node),
                            Err(mut e) => errors.append(&mut e),
                        },
                        "confidence_floor" => match self.parse_named_params_section("confidence_floor") {
                            Ok(params) => nodes.push(AstNode::ConfidenceFloor { entries: params }),
                            Err(mut e) => errors.append(&mut e),
                        },
                        other => {
                            errors.push(format!("line {}: unknown section '{}'", self.current_line(), other));
                            // Try to skip to next section by finding the matching }
                            self.skip_to_closing_brace();
                        }
                    }
                }

                Token::Constraint(_) => {
                    // Stray constraint comment outside a section — skip
                    self.advance();
                }

                other => {
                    errors.push(format!("line {}: unexpected token {:?}", self.current_line(), other));
                    self.advance();
                }
            }
        }

        if errors.is_empty() {
            Ok(nodes)
        } else {
            Err(errors)
        }
    }

    fn skip_to_closing_brace(&mut self) {
        let mut depth = 0;
        loop {
            match self.peek() {
                Token::Eof => break,
                Token::LBrace => { depth += 1; self.advance(); }
                Token::RBrace => {
                    self.advance();
                    depth -= 1;
                    if depth <= 0 { break; }
                }
                _ => { self.advance(); }
            }
        }
    }

    // --- Section parsers ---

    fn parse_version(&mut self) -> Result<AstNode, Vec<String>> {
        self.expect_token(&Token::At)?;
        let (ident, line) = self.expect_identifier()?;
        if ident != "version" {
            return Err(vec![format!("line {}: expected 'version' after @, got '{}'", line, ident)]);
        }
        let (num, line) = self.expect_number()?;
        if num < 0.0 || num.fract() != 0.0 {
            return Err(vec![format!("line {}: version must be a positive integer", line)]);
        }
        Ok(AstNode::Version(num as u32))
    }

    fn parse_calibration(&mut self) -> Result<AstNode, Vec<String>> {
        // calibration <name> { ... }
        self.advance(); // consume 'calibration'
        let (name, _) = self.expect_identifier()?;
        self.expect_token(&Token::LBrace)?;
        let mut params = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Token::RBrace => { self.advance(); break; }
                Token::Eof => return Err(vec![format!("line {}: unexpected EOF in calibration block", self.current_line())]),
                Token::Constraint(_) => { self.advance(); } // skip constraint comments in calibration
                _ => {
                    params.push(self.parse_param()?);
                }
            }
        }
        Ok(AstNode::Calibration { name, params })
    }

    fn parse_confirmation_gate(&mut self) -> Result<AstNode, Vec<String>> {
        // confirmation_gate { N => (f, f) ... }
        self.advance(); // consume 'confirmation_gate'
        self.expect_token(&Token::LBrace)?;
        let mut entries = Vec::new();
        let mut lines = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Token::RBrace => { self.advance(); break; }
                Token::Eof => return Err(vec![format!("line {}: unexpected EOF in confirmation_gate", self.current_line())]),
                Token::Constraint(_) => { self.advance(); }
                _ => {
                    let (key, line) = self.expect_u8()?;
                    self.expect_token(&Token::FatArrow)?;
                    self.expect_token(&Token::LParen)?;
                    let (a, _) = self.expect_number()?;
                    self.expect_token(&Token::Comma)?;
                    let (b, _) = self.expect_number()?;
                    self.expect_token(&Token::RParen)?;
                    entries.push((key, (a, b)));
                    lines.push(line);
                }
            }
        }
        Ok(AstNode::ConfirmationGate { entries, lines })
    }

    fn parse_freshness_tiers(&mut self) -> Result<AstNode, Vec<String>> {
        // freshness_tiers { hours => mult ... _ => default }
        self.advance(); // consume 'freshness_tiers'
        self.expect_token(&Token::LBrace)?;
        let mut tiers = Vec::new();
        let mut default: Option<f64> = None;
        let mut tier_lines = Vec::new();
        let mut default_line = 0;
        loop {
            self.skip_newlines();
            match self.peek() {
                Token::RBrace => { self.advance(); break; }
                Token::Eof => return Err(vec![format!("line {}: unexpected EOF in freshness_tiers", self.current_line())]),
                Token::Underscore => {
                    default_line = self.current_line();
                    self.advance();
                    self.expect_token(&Token::FatArrow)?;
                    let (val, _) = self.expect_number()?;
                    default = Some(val);
                }
                Token::Constraint(_) => { self.advance(); }
                _ => {
                    let (hours, line) = self.expect_number()?;
                    self.expect_token(&Token::FatArrow)?;
                    let (mult, _) = self.expect_number()?;
                    tiers.push((hours, mult));
                    tier_lines.push(line);
                }
            }
        }
        match default {
            Some(d) => Ok(AstNode::FreshnessTiers { tiers, default: d, tier_lines, default_line }),
            None => Err(vec![format!("line {}: freshness_tiers missing wildcard '_' default entry", self.current_line())]),
        }
    }

    fn parse_weights(&mut self) -> Result<AstNode, Vec<String>> {
        // weights <name> { ... }
        self.advance(); // consume 'weights'
        let (name, _) = self.expect_identifier()?;
        self.expect_token(&Token::LBrace)?;
        let mut params = Vec::new();
        let mut constraints = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Token::RBrace => { self.advance(); break; }
                Token::Eof => return Err(vec![format!("line {}: unexpected EOF in weights block", self.current_line())]),
                Token::Constraint(text) => {
                    let line = self.current_line();
                    let text = text.clone();
                    self.advance();
                    if let Some(c) = parse_constraint_text(&text, line) {
                        constraints.push(c);
                    }
                }
                _ => {
                    params.push(self.parse_param()?);
                }
            }
        }
        Ok(AstNode::Weights { name, params, constraints })
    }

    fn parse_bounds(&mut self) -> Result<AstNode, Vec<String>> {
        // bounds { name: [min, max] | name: value  # [range] }
        self.advance(); // consume 'bounds'
        self.expect_token(&Token::LBrace)?;
        let mut entries = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Token::RBrace => { self.advance(); break; }
                Token::Eof => return Err(vec![format!("line {}: unexpected EOF in bounds block", self.current_line())]),
                Token::Constraint(_) => { self.advance(); }
                _ => {
                    let (name, line) = self.expect_identifier()?;
                    self.expect_token(&Token::Colon)?;
                    // Check if next token is [ (range value) or a number (scalar)
                    self.skip_newlines();
                    match self.peek() {
                        Token::LBracket => {
                            self.advance(); // consume [
                            let (min, _) = self.expect_number()?;
                            self.expect_token(&Token::Comma)?;
                            let (max, _) = self.expect_number()?;
                            self.expect_token(&Token::RBracket)?;
                            entries.push(BoundsEntry::Range { name, min, max, line });
                        }
                        _ => {
                            let (value, _) = self.expect_number()?;
                            let range = self.try_parse_range();
                            entries.push(BoundsEntry::Scalar { name, value, range, line });
                        }
                    }
                }
            }
        }
        Ok(AstNode::Bounds { entries })
    }

    fn parse_semantic_boost(&mut self) -> Result<AstNode, Vec<String>> {
        // semantic_boost { name: [min, max] | name: value  # [range] }
        // Same structure as bounds but produces SemanticBoost node with named params
        self.advance(); // consume 'semantic_boost'
        self.expect_token(&Token::LBrace)?;
        let mut entries = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Token::RBrace => { self.advance(); break; }
                Token::Eof => return Err(vec![format!("line {}: unexpected EOF in semantic_boost block", self.current_line())]),
                Token::Constraint(_) => { self.advance(); }
                _ => {
                    let (name, line) = self.expect_identifier()?;
                    self.expect_token(&Token::Colon)?;
                    self.skip_newlines();
                    match self.peek() {
                        Token::LBracket => {
                            // Range value like [0.5, 1.5] — store as two params: name_min, name_max
                            self.advance(); // consume [
                            let (min, _) = self.expect_number()?;
                            self.expect_token(&Token::Comma)?;
                            let (max, _) = self.expect_number()?;
                            self.expect_token(&Token::RBracket)?;
                            entries.push(Param { name: format!("{}_min", name), value: min, range: None, line });
                            entries.push(Param { name: format!("{}_max", name), value: max, range: None, line });
                        }
                        _ => {
                            let (value, _) = self.expect_number()?;
                            let range = self.try_parse_range();
                            entries.push(Param { name, value, range, line });
                        }
                    }
                }
            }
        }
        Ok(AstNode::SemanticBoost { entries })
    }

    fn parse_confidence_bonuses(&mut self) -> Result<AstNode, Vec<String>> {
        // confidence_bonuses { N => f ... }
        self.advance(); // consume 'confidence_bonuses'
        self.expect_token(&Token::LBrace)?;
        let mut entries = Vec::new();
        let mut lines = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Token::RBrace => { self.advance(); break; }
                Token::Eof => return Err(vec![format!("line {}: unexpected EOF in confidence_bonuses", self.current_line())]),
                Token::Constraint(_) => { self.advance(); }
                _ => {
                    let (key, line) = self.expect_u8()?;
                    self.expect_token(&Token::FatArrow)?;
                    let (value, _) = self.expect_number()?;
                    entries.push((key, value));
                    lines.push(line);
                }
            }
        }
        Ok(AstNode::ConfidenceBonuses { entries, lines })
    }

    /// Generic parser for simple `section_name { key: value # [range] ... }` sections
    fn parse_named_params_section(&mut self, _section_name: &str) -> Result<Vec<Param>, Vec<String>> {
        self.advance(); // consume section identifier
        self.expect_token(&Token::LBrace)?;
        let mut params = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Token::RBrace => { self.advance(); break; }
                Token::Eof => return Err(vec![format!("line {}: unexpected EOF in section", self.current_line())]),
                Token::Constraint(_) => { self.advance(); }
                _ => {
                    params.push(self.parse_param()?);
                }
            }
        }
        Ok(params)
    }
}

/// Parse constraint text like `interest_share + keyword_share == 1.0`
fn parse_constraint_text(text: &str, line: usize) -> Option<Constraint> {
    // Format: field1 + field2 + ... == value
    let parts: Vec<&str> = text.split("==").collect();
    if parts.len() != 2 {
        return None;
    }
    let lhs = parts[0].trim();
    let rhs = parts[1].trim();

    let expected_sum: f64 = match rhs.parse() {
        Ok(v) => v,
        Err(_) => return None,
    };

    let fields: Vec<String> = lhs.split('+')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if fields.is_empty() {
        return None;
    }

    Some(Constraint { fields, expected_sum, line })
}

// ============================================================================
// Stage 3: Validator
// ============================================================================

fn validate(nodes: &[AstNode]) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for node in nodes {
        match node {
            AstNode::Calibration { params, .. } => {
                validate_param_ranges(params, &mut errors);
            }

            AstNode::ConfirmationGate { entries, lines } => {
                // Completeness: must have 0-4
                let keys: Vec<u8> = entries.iter().map(|e| e.0).collect();
                for required in 0..=4u8 {
                    if !keys.contains(&required) {
                        let line = lines.first().copied().unwrap_or(0);
                        errors.push(ValidationError {
                            line,
                            message: format!("confirmation_gate missing entry for {} signals", required),
                        });
                    }
                }
                // Monotonicity: gate multipliers (first element of tuple) must be non-decreasing
                // Sort by key first to check in order
                let mut sorted: Vec<_> = entries.clone();
                sorted.sort_by_key(|e| e.0);
                for i in 1..sorted.len() {
                    let (prev_key, (prev_mult, _)) = sorted[i - 1];
                    let (cur_key, (cur_mult, _)) = sorted[i];
                    if cur_mult < prev_mult {
                        let line = lines.get(i).copied().unwrap_or(0);
                        errors.push(ValidationError {
                            line,
                            message: format!(
                                "confirmation_gate multiplier for {} signals ({}) is less than for {} signals ({}); must be non-decreasing",
                                cur_key, cur_mult, prev_key, prev_mult
                            ),
                        });
                    }
                }
                // Also check floor values are non-decreasing
                for i in 1..sorted.len() {
                    let (prev_key, (_, prev_floor)) = sorted[i - 1];
                    let (cur_key, (_, cur_floor)) = sorted[i];
                    if cur_floor < prev_floor {
                        let line = lines.get(i).copied().unwrap_or(0);
                        errors.push(ValidationError {
                            line,
                            message: format!(
                                "confirmation_gate floor for {} signals ({}) is less than for {} signals ({}); must be non-decreasing",
                                cur_key, cur_floor, prev_key, prev_floor
                            ),
                        });
                    }
                }
            }

            AstNode::SignalThresholds { entries } => {
                validate_param_ranges(entries, &mut errors);
            }

            AstNode::FreshnessTiers { tiers, tier_lines, .. } => {
                // Monotonicity: multipliers must be non-increasing (as age increases, mult decreases)
                for i in 1..tiers.len() {
                    let (prev_hours, prev_mult) = tiers[i - 1];
                    let (cur_hours, cur_mult) = tiers[i];
                    if cur_mult > prev_mult {
                        let line = tier_lines.get(i).copied().unwrap_or(0);
                        errors.push(ValidationError {
                            line,
                            message: format!(
                                "freshness tier multiplier at {}h ({}) is greater than at {}h ({}); must be non-increasing",
                                cur_hours, cur_mult, prev_hours, prev_mult
                            ),
                        });
                    }
                }
            }

            AstNode::Weights { params, constraints, .. } => {
                validate_param_ranges(params, &mut errors);
                // Validate sum constraints
                for constraint in constraints {
                    let mut sum = 0.0_f64;
                    let mut all_found = true;
                    for field in &constraint.fields {
                        match params.iter().find(|p| p.name == *field) {
                            Some(p) => sum += p.value,
                            None => {
                                errors.push(ValidationError {
                                    line: constraint.line,
                                    message: format!("constraint references unknown field '{}'", field),
                                });
                                all_found = false;
                            }
                        }
                    }
                    if all_found {
                        let diff = (sum - constraint.expected_sum).abs();
                        if diff > 1e-6 {
                            errors.push(ValidationError {
                                line: constraint.line,
                                message: format!(
                                    "constraint {} == {} failed: actual sum is {}",
                                    constraint.fields.join(" + "),
                                    constraint.expected_sum,
                                    sum
                                ),
                            });
                        }
                    }
                }
            }

            AstNode::Bounds { entries } => {
                for entry in entries {
                    match entry {
                        BoundsEntry::Range { name, min, max, line } => {
                            if min > max {
                                errors.push(ValidationError {
                                    line: *line,
                                    message: format!("bounds '{}': min ({}) > max ({})", name, min, max),
                                });
                            }
                        }
                        BoundsEntry::Scalar { name, value, range, line } => {
                            if let Some((rmin, rmax)) = range {
                                if *value < *rmin || *value > *rmax {
                                    errors.push(ValidationError {
                                        line: *line,
                                        message: format!(
                                            "bounds '{}': value {} outside range [{}, {}]",
                                            name, value, rmin, rmax
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            AstNode::ConfidenceBonuses { entries, lines } => {
                // Completeness: must have 0-4
                let keys: Vec<u8> = entries.iter().map(|e| e.0).collect();
                for required in 0..=4u8 {
                    if !keys.contains(&required) {
                        let line = lines.first().copied().unwrap_or(0);
                        errors.push(ValidationError {
                            line,
                            message: format!("confidence_bonuses missing entry for level {}", required),
                        });
                    }
                }
                // Monotonicity: bonuses should be non-decreasing
                let mut sorted: Vec<_> = entries.clone();
                sorted.sort_by_key(|e| e.0);
                for i in 1..sorted.len() {
                    let (prev_key, prev_val) = sorted[i - 1];
                    let (cur_key, cur_val) = sorted[i];
                    if cur_val < prev_val {
                        let line = lines.get(i).copied().unwrap_or(0);
                        errors.push(ValidationError {
                            line,
                            message: format!(
                                "confidence_bonuses: bonus for level {} ({}) is less than level {} ({}); must be non-decreasing",
                                cur_key, cur_val, prev_key, prev_val
                            ),
                        });
                    }
                }
            }

            AstNode::SpecificityWeights { entries } |
            AstNode::KeywordAceBoost { entries } |
            AstNode::Serendipity { entries } |
            AstNode::PriorityCaps { entries } |
            AstNode::QualityFloor { entries } |
            AstNode::SemanticBoost { entries } |
            AstNode::ConfidenceFloor { entries } => {
                validate_param_ranges(entries, &mut errors);
            }

            AstNode::Version(_) => {}
        }
    }

    errors
}

fn validate_param_ranges(params: &[Param], errors: &mut Vec<ValidationError>) {
    for param in params {
        if let Some((min, max)) = param.range {
            if param.value < min || param.value > max {
                errors.push(ValidationError {
                    line: param.line,
                    message: format!(
                        "'{}': value {} outside range [{}, {}]",
                        param.name, param.value, min, max
                    ),
                });
            }
        }
    }
}

// ============================================================================
// Stage 4: Code Generator
// ============================================================================

fn to_screaming_snake(s: &str) -> String {
    s.to_uppercase()
}

fn format_f32(v: f64) -> String {
    // Ensure we always emit a decimal point for f32 literals
    let s = format!("{}", v);
    if s.contains('.') {
        s
    } else {
        format!("{}.0", s)
    }
}

fn generate(nodes: &[AstNode]) -> String {
    let mut out = String::with_capacity(4096);

    out.push_str("// AUTO-GENERATED from scoring/pipeline.scoring — DO NOT EDIT\n");
    out.push_str("#[allow(dead_code)]\n");
    out.push_str("const _: () = ();\n\n");

    for node in nodes {
        match node {
            AstNode::Version(_) => {
                // Version is metadata, no code generated
            }

            AstNode::Calibration { name, params } => {
                out.push_str("// === Calibration ===\n");
                let prefix = to_screaming_snake(name);
                for param in params {
                    let const_name = format!("{}_{}", prefix, to_screaming_snake(&param.name));
                    out.push_str(&format!(
                        "pub const {}: f32 = {};\n",
                        const_name, format_f32(param.value)
                    ));
                }
                out.push('\n');
            }

            AstNode::ConfirmationGate { entries, .. } => {
                out.push_str("// === Confirmation Gate ===\n");
                let mut sorted: Vec<_> = entries.clone();
                sorted.sort_by_key(|e| e.0);
                let len = sorted.len();
                out.push_str(&format!(
                    "pub const CONFIRMATION_GATE: [(f32, f32); {}] = [\n",
                    len
                ));
                for (key, (mult, floor)) in &sorted {
                    out.push_str(&format!(
                        "    ({}, {}), // {} signal{}\n",
                        format_f32(*mult),
                        format_f32(*floor),
                        key,
                        if *key == 1 { "" } else { "s" }
                    ));
                }
                out.push_str("];\n\n");
            }

            AstNode::SignalThresholds { entries } => {
                out.push_str("// === Signal Thresholds ===\n");
                for param in entries {
                    let const_name = format!("{}_THRESHOLD", to_screaming_snake(&param.name));
                    out.push_str(&format!(
                        "pub const {}: f32 = {};\n",
                        const_name, format_f32(param.value)
                    ));
                }
                out.push('\n');
            }

            AstNode::FreshnessTiers { tiers, default, .. } => {
                out.push_str("// === Freshness Tiers ===\n");
                let len = tiers.len();
                out.push_str(&format!(
                    "pub const FRESHNESS_TIERS: [(f32, f32); {}] = [\n",
                    len
                ));
                for (hours, mult) in tiers {
                    out.push_str(&format!(
                        "    ({}, {}),\n",
                        format_f32(*hours),
                        format_f32(*mult)
                    ));
                }
                out.push_str("];\n");
                out.push_str(&format!(
                    "pub const FRESHNESS_DEFAULT: f32 = {};\n\n",
                    format_f32(*default)
                ));
                // Generate the lookup function
                out.push_str("#[inline]\n");
                out.push_str("pub fn freshness_multiplier(age_hours: f32) -> f32 {\n");
                out.push_str("    for &(threshold, mult) in &FRESHNESS_TIERS {\n");
                out.push_str("        if age_hours < threshold {\n");
                out.push_str("            return mult;\n");
                out.push_str("        }\n");
                out.push_str("    }\n");
                out.push_str("    FRESHNESS_DEFAULT\n");
                out.push_str("}\n\n");
            }

            AstNode::Weights { name, params, .. } => {
                let prefix = to_screaming_snake(name);
                out.push_str(&format!("// === Weights: {} ===\n", name));
                for param in params {
                    let const_name = format!("{}_{}", prefix, to_screaming_snake(&param.name));
                    out.push_str(&format!(
                        "pub const {}: f32 = {};\n",
                        const_name, format_f32(param.value)
                    ));
                }
                out.push('\n');
            }

            AstNode::Bounds { entries } => {
                out.push_str("// === Bounds ===\n");
                for entry in entries {
                    match entry {
                        BoundsEntry::Range { name, min, max, .. } => {
                            let const_name = format!("{}_RANGE", to_screaming_snake(name));
                            out.push_str(&format!(
                                "pub const {}: (f32, f32) = ({}, {});\n",
                                const_name, format_f32(*min), format_f32(*max)
                            ));
                        }
                        BoundsEntry::Scalar { name, value, .. } => {
                            let const_name = to_screaming_snake(name);
                            out.push_str(&format!(
                                "pub const {}: f32 = {};\n",
                                const_name, format_f32(*value)
                            ));
                        }
                    }
                }
                out.push('\n');
            }

            AstNode::ConfidenceBonuses { entries, .. } => {
                out.push_str("// === Confidence Bonuses ===\n");
                let mut sorted: Vec<_> = entries.clone();
                sorted.sort_by_key(|e| e.0);
                let len = sorted.len();
                out.push_str(&format!(
                    "pub const CONFIDENCE_BONUSES: [f32; {}] = [",
                    len
                ));
                for (i, (_, val)) in sorted.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&format_f32(*val));
                }
                out.push_str("];\n\n");
            }

            AstNode::SpecificityWeights { entries } => {
                out.push_str("// === Specificity Weights ===\n");
                for param in entries {
                    let const_name = format!("SPECIFICITY_{}", to_screaming_snake(&param.name));
                    out.push_str(&format!(
                        "pub const {}: f32 = {};\n",
                        const_name, format_f32(param.value)
                    ));
                }
                out.push('\n');
            }

            AstNode::KeywordAceBoost { entries } => {
                out.push_str("// === Keyword ACE Boost ===\n");
                for param in entries {
                    let const_name = format!("ACE_{}", to_screaming_snake(&param.name));
                    out.push_str(&format!(
                        "pub const {}: f32 = {};\n",
                        const_name, format_f32(param.value)
                    ));
                }
                out.push('\n');
            }

            AstNode::Serendipity { entries } => {
                out.push_str("// === Serendipity ===\n");
                for param in entries {
                    let const_name = format!("SERENDIPITY_{}", to_screaming_snake(&param.name));
                    out.push_str(&format!(
                        "pub const {}: f32 = {};\n",
                        const_name, format_f32(param.value)
                    ));
                }
                out.push('\n');
            }

            AstNode::PriorityCaps { entries } => {
                out.push_str("// === Priority Caps ===\n");
                for param in entries {
                    let const_name = to_screaming_snake(&param.name);
                    out.push_str(&format!(
                        "pub const {}: f32 = {};\n",
                        const_name, format_f32(param.value)
                    ));
                }
                out.push('\n');
            }

            AstNode::QualityFloor { entries } => {
                out.push_str("// === Quality Floor ===\n");
                for param in entries {
                    let const_name = format!("QUALITY_FLOOR_{}", to_screaming_snake(&param.name));
                    out.push_str(&format!(
                        "pub const {}: f32 = {};\n",
                        const_name, format_f32(param.value)
                    ));
                }
                out.push('\n');
            }

            AstNode::SemanticBoost { entries } => {
                out.push_str("// === Semantic Boost ===\n");
                for param in entries {
                    let const_name = format!("SEMANTIC_BOOST_{}", to_screaming_snake(&param.name));
                    out.push_str(&format!(
                        "pub const {}: f32 = {};\n",
                        const_name, format_f32(param.value)
                    ));
                }
                out.push('\n');
            }

            AstNode::ConfidenceFloor { entries } => {
                out.push_str("// === Confidence Floor ===\n");
                for param in entries {
                    let const_name = format!("CONFIDENCE_FLOOR_{}", to_screaming_snake(&param.name));
                    out.push_str(&format!(
                        "pub const {}: f32 = {};\n",
                        const_name, format_f32(param.value)
                    ));
                }
                out.push('\n');
            }
        }
    }

    out
}

// ============================================================================
// Public API
// ============================================================================

/// Compile a `.scoring` DSL file into a generated Rust source file.
///
/// Called from `build.rs` to produce `scoring_config.rs` at build time.
///
/// # Arguments
/// * `input` - Contents of the `.scoring` DSL file
/// * `output_path` - Where to write the generated Rust source
///
/// # Returns
/// * `Ok(())` on success
/// * `Err(Vec<String>)` with human-readable error messages (including line numbers)
pub fn compile_scoring_dsl(input: &str, output_path: &std::path::Path) -> Result<(), Vec<String>> {
    // Stage 1: Lex
    let tokens = lex(input)?;

    // Stage 2: Parse
    let mut parser = Parser::new(tokens);
    let ast = parser.parse()?;

    // Stage 3: Validate
    let errors = validate(&ast);
    if !errors.is_empty() {
        return Err(errors.into_iter().map(|e| e.to_string()).collect());
    }

    // Stage 4: Generate
    let code = generate(&ast);
    std::fs::write(output_path, code).map_err(|e| vec![e.to_string()])?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DSL: &str = r#"
@version 1

calibration sigmoid {
    center: 0.48    # [0.30, 0.70]
    scale: 12.0     # [5.0, 30.0]
}

confirmation_gate {
    0 => (0.25, 0.20)
    1 => (0.45, 0.32)
    2 => (1.00, 0.80)
    3 => (1.10, 0.92)
    4 => (1.20, 1.00)
}

signal_thresholds {
    context:  0.45   # [0.20, 0.80]
    interest: 0.50   # [0.20, 0.80]
    keyword:  0.60   # [0.30, 0.90]
    semantic: 0.12   # [0.05, 0.50]
    feedback: 0.05   # [0.01, 0.30]
    affinity: 1.15   # [1.01, 2.00]
}

freshness_tiers {
    3.0   => 1.10
    12.0  => 1.08
    24.0  => 1.05
    72.0  => 1.00
    168.0 => 0.92
    720.0 => 0.85
    _     => 0.80
}

weights base_both {
    context_base: 0.15    # [0.05, 0.50]
    context_scale: 0.40   # [0.10, 0.60]
    context_max: 0.55     # [0.30, 0.80]
    interest_share: 0.55  # [0.30, 0.80]
    keyword_share: 0.45   # [0.20, 0.70]
    # constraint: interest_share + keyword_share == 1.0
}

weights interest_only {
    interest_w: 0.45     # [0.20, 0.70]
    keyword_w: 0.35      # [0.15, 0.60]
    semantic_mult: 1.2   # [0.5, 2.0]
}

bounds {
    affinity_mult: [0.3, 1.7]
    affinity_effect: 0.7
    anti_penalty_max: 0.7
    feedback_cap: [-0.20, 0.20]
    feedback_scale: 0.15
    source_quality_cap: [-0.10, 0.10]
    source_quality_mult: 0.10
    off_domain_penalty: 0.12
    relevance_default: 0.50    # [0.30, 0.70]
}

confidence_bonuses {
    0 => -0.15
    1 => 0.0
    2 => 0.10
    3 => 0.15
    4 => 0.20
}

specificity_weights {
    broad: 0.25
    single_word: 0.60
    multi_word: 1.00
    embedding_broad: 0.40
}

keyword_ace_boost {
    topic_confidence_default: 0.5
    active_topic_boost: 0.15
    detected_tech_boost: 0.12
    max_boost: 0.3
}

serendipity {
    min_score: 0.15
    min_axis_score: 0.2
    max_items: 5
}

priority_caps {
    low_score_cap: 0.35
    medium_score_cap: 0.45
    high_score_floor: 0.70
}
"#;

    #[test]
    fn test_lex_basic() {
        let tokens = lex(SAMPLE_DSL).expect("lexing should succeed");
        // Check that we got tokens and no errors
        assert!(!tokens.is_empty());
        // Check version token sequence: At, Identifier("version"), Number(1)
        let non_newline: Vec<_> = tokens.iter()
            .filter(|t| !matches!(t.token, Token::Newline))
            .collect();
        assert!(matches!(non_newline[0].token, Token::At));
        assert!(matches!(&non_newline[1].token, Token::Identifier(s) if s == "version"));
        assert!(matches!(non_newline[2].token, Token::Number(n) if (n - 1.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_parse_full() {
        let tokens = lex(SAMPLE_DSL).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().expect("parsing should succeed");

        // Should have: Version, Calibration, ConfirmationGate, SignalThresholds,
        // FreshnessTiers, Weights(base_both), Weights(interest_only), Bounds,
        // ConfidenceBonuses, SpecificityWeights, KeywordAceBoost, Serendipity, PriorityCaps
        assert_eq!(ast.len(), 13);

        // Check version
        assert!(matches!(&ast[0], AstNode::Version(1)));

        // Check calibration
        if let AstNode::Calibration { name, params } = &ast[1] {
            assert_eq!(name, "sigmoid");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "center");
            assert!((params[0].value - 0.48).abs() < f64::EPSILON);
            assert_eq!(params[0].range, Some((0.30, 0.70)));
        } else {
            panic!("expected Calibration");
        }

        // Check confirmation gate
        if let AstNode::ConfirmationGate { entries, .. } = &ast[2] {
            assert_eq!(entries.len(), 5);
        } else {
            panic!("expected ConfirmationGate");
        }

        // Check freshness tiers
        if let AstNode::FreshnessTiers { tiers, default, .. } = &ast[4] {
            assert_eq!(tiers.len(), 6);
            assert!((default - 0.80).abs() < f64::EPSILON);
        } else {
            panic!("expected FreshnessTiers");
        }

        // Check bounds
        if let AstNode::Bounds { entries } = &ast[7] {
            assert_eq!(entries.len(), 9);
            // First entry should be a range
            assert!(matches!(&entries[0], BoundsEntry::Range { name, .. } if name == "affinity_mult"));
            // relevance_default should be scalar with range annotation
            assert!(matches!(&entries[8], BoundsEntry::Scalar { name, range: Some(_), .. } if name == "relevance_default"));
        } else {
            panic!("expected Bounds");
        }
    }

    #[test]
    fn test_validate_passes() {
        let tokens = lex(SAMPLE_DSL).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        let errors = validate(&ast);
        assert!(errors.is_empty(), "validation errors: {:?}", errors);
    }

    #[test]
    fn test_validate_range_violation() {
        let input = r#"
calibration sigmoid {
    center: 0.90    # [0.30, 0.70]
    scale: 12.0
}
"#;
        let tokens = lex(input).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        let errors = validate(&ast);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("outside range"));
    }

    #[test]
    fn test_validate_constraint_failure() {
        let input = r#"
weights test {
    a: 0.30
    b: 0.40
    # constraint: a + b == 1.0
}
"#;
        let tokens = lex(input).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        let errors = validate(&ast);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("constraint"));
    }

    #[test]
    fn test_validate_constraint_passes() {
        let input = r#"
weights test {
    a: 0.60
    b: 0.40
    # constraint: a + b == 1.0
}
"#;
        let tokens = lex(input).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        let errors = validate(&ast);
        assert!(errors.is_empty(), "errors: {:?}", errors);
    }

    #[test]
    fn test_validate_freshness_monotonicity_violation() {
        let input = r#"
freshness_tiers {
    3.0   => 1.00
    12.0  => 1.10
    _     => 0.80
}
"#;
        let tokens = lex(input).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        let errors = validate(&ast);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("non-increasing"));
    }

    #[test]
    fn test_validate_confirmation_gate_completeness() {
        let input = r#"
confirmation_gate {
    0 => (0.25, 0.20)
    2 => (1.00, 0.80)
    4 => (1.20, 1.00)
}
"#;
        let tokens = lex(input).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        let errors = validate(&ast);
        // Should report missing entries for 1 and 3
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().any(|e| e.message.contains("1 signals")));
        assert!(errors.iter().any(|e| e.message.contains("3 signals")));
    }

    #[test]
    fn test_validate_confidence_completeness() {
        let input = r#"
confidence_bonuses {
    0 => -0.15
    4 => 0.20
}
"#;
        let tokens = lex(input).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        let errors = validate(&ast);
        assert_eq!(errors.len(), 3); // missing 1, 2, 3
    }

    #[test]
    fn test_validate_gate_monotonicity() {
        let input = r#"
confirmation_gate {
    0 => (1.00, 0.80)
    1 => (0.50, 0.32)
    2 => (1.00, 0.80)
    3 => (1.10, 0.92)
    4 => (1.20, 1.00)
}
"#;
        let tokens = lex(input).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        let errors = validate(&ast);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.message.contains("non-decreasing")));
    }

    #[test]
    fn test_generate_output() {
        let tokens = lex(SAMPLE_DSL).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        let output = generate(&ast);

        // Check key constants are present
        assert!(output.contains("pub const SIGMOID_CENTER: f32 = 0.48;"));
        assert!(output.contains("pub const SIGMOID_SCALE: f32 = 12.0;"));
        assert!(output.contains("pub const CONFIRMATION_GATE: [(f32, f32); 5]"));
        assert!(output.contains("pub const CONTEXT_THRESHOLD: f32 = 0.45;"));
        assert!(output.contains("pub const FRESHNESS_TIERS: [(f32, f32); 6]"));
        assert!(output.contains("pub const FRESHNESS_DEFAULT: f32 = 0.8"));
        assert!(output.contains("pub fn freshness_multiplier(age_hours: f32) -> f32"));
        assert!(output.contains("pub const BASE_BOTH_CONTEXT_BASE: f32 = 0.15;"));
        assert!(output.contains("pub const INTEREST_ONLY_INTEREST_W: f32 = 0.45;"));
        assert!(output.contains("pub const AFFINITY_MULT_RANGE: (f32, f32) = (0.3, 1.7);"));
        assert!(output.contains("pub const AFFINITY_EFFECT: f32 = 0.7;"));
        assert!(output.contains("pub const FEEDBACK_CAP_RANGE: (f32, f32) = (-0.2, 0.2);"));
        assert!(output.contains("pub const CONFIDENCE_BONUSES: [f32; 5]"));
        assert!(output.contains("pub const SPECIFICITY_BROAD: f32 = 0.25;"));
        assert!(output.contains("pub const ACE_TOPIC_CONFIDENCE_DEFAULT: f32 = 0.5;"));
        assert!(output.contains("pub const SERENDIPITY_MIN_SCORE: f32 = 0.15;"));
        assert!(output.contains("pub const LOW_SCORE_CAP: f32 = 0.35;"));
        assert!(output.contains("AUTO-GENERATED"));
    }

    #[test]
    fn test_negative_numbers() {
        let input = r#"
confidence_bonuses {
    0 => -0.15
    1 => 0.0
    2 => 0.10
    3 => 0.15
    4 => 0.20
}
"#;
        let tokens = lex(input).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        if let AstNode::ConfidenceBonuses { entries, .. } = &ast[0] {
            assert_eq!(entries[0].1, -0.15);
        } else {
            panic!("expected ConfidenceBonuses");
        }
    }

    #[test]
    fn test_negative_range_bounds() {
        let input = r#"
bounds {
    feedback_cap: [-0.20, 0.20]
}
"#;
        let tokens = lex(input).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        if let AstNode::Bounds { entries } = &ast[0] {
            if let BoundsEntry::Range { min, max, .. } = &entries[0] {
                assert!((*min - (-0.20)).abs() < f64::EPSILON);
                assert!((*max - 0.20).abs() < f64::EPSILON);
            } else {
                panic!("expected Range entry");
            }
        } else {
            panic!("expected Bounds");
        }
    }

    #[test]
    fn test_empty_section() {
        let input = r#"
serendipity {
}
"#;
        let tokens = lex(input).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        if let AstNode::Serendipity { entries } = &ast[0] {
            assert!(entries.is_empty());
        } else {
            panic!("expected Serendipity");
        }
    }

    #[test]
    fn test_freshness_missing_default() {
        let input = r#"
freshness_tiers {
    3.0 => 1.10
    12.0 => 1.08
}
"#;
        let tokens = lex(input).unwrap();
        let mut parser = Parser::new(tokens);
        let result = parser.parse();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("wildcard")));
    }

    #[test]
    fn test_compile_scoring_dsl_roundtrip() {
        let dir = std::env::temp_dir();
        let output_path = dir.join("test_scoring_config.rs");
        let result = compile_scoring_dsl(SAMPLE_DSL, &output_path);
        assert!(result.is_ok(), "compile failed: {:?}", result.err());

        let generated = std::fs::read_to_string(&output_path).unwrap();
        assert!(generated.contains("pub const SIGMOID_CENTER"));
        assert!(generated.contains("pub const CONFIRMATION_GATE"));
        assert!(generated.contains("pub fn freshness_multiplier"));

        // Cleanup
        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn test_bounds_range_invalid() {
        let input = r#"
bounds {
    bad_range: [1.0, 0.5]
}
"#;
        let tokens = lex(input).unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();
        let errors = validate(&ast);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("min") && errors[0].message.contains("max"));
    }
}
