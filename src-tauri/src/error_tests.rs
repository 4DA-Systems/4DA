#[cfg(test)]
mod tests {
    use crate::error::{FourDaError, Result, ResultExt};

    // ========================================================================
    // FourDaError construction and Display
    // ========================================================================

    #[test]
    fn test_error_variants_display() {
        let cases: Vec<(FourDaError, &str)> = vec![
            (
                FourDaError::Config("bad provider".into()),
                "Config error: bad provider",
            ),
            (
                FourDaError::NotInitialized("ACE not ready".into()),
                "ACE not ready",
            ),
            (
                FourDaError::Analysis("timed out".into()),
                "Analysis error: timed out",
            ),
            (
                FourDaError::Llm("rate limited".into()),
                "LLM error: rate limited",
            ),
            (
                FourDaError::Internal("something broke".into()),
                "something broke",
            ),
        ];

        for (err, expected) in cases {
            assert_eq!(err.to_string(), expected, "Display mismatch for {:?}", err);
        }
    }

    #[test]
    fn test_error_from_string() {
        let err: FourDaError = "test error".to_string().into();
        match &err {
            FourDaError::Internal(msg) => assert_eq!(msg, "test error"),
            other => panic!("Expected Internal, got {:?}", other),
        }
    }

    #[test]
    fn test_error_from_str() {
        let err: FourDaError = "static error".into();
        assert_eq!(err.to_string(), "static error");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: FourDaError = io_err.into();
        match &err {
            FourDaError::Io(_) => {}
            other => panic!("Expected Io, got {:?}", other),
        }
        assert!(err.to_string().contains("file missing"));
    }

    #[test]
    fn test_error_from_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("{{bad json}").unwrap_err();
        let err: FourDaError = json_err.into();
        match &err {
            FourDaError::Json(_) => {}
            other => panic!("Expected Json, got {:?}", other),
        }
        assert!(err.to_string().contains("JSON error"));
    }

    // ========================================================================
    // Serialization (required for Tauri IPC)
    // ========================================================================

    #[test]
    fn test_error_serializes_as_string() {
        let err = FourDaError::Config("invalid API key".into());
        let json = serde_json::to_string(&err).expect("serialize error");
        // FourDaError serializes as a plain string (its Display output)
        assert_eq!(json, "\"Config error: invalid API key\"");
    }

    #[test]
    fn test_all_variants_serialize() {
        let errors = vec![
            FourDaError::Config("c".into()),
            FourDaError::NotInitialized("n".into()),
            FourDaError::Analysis("a".into()),
            FourDaError::Llm("l".into()),
            FourDaError::Internal("i".into()),
            FourDaError::Io(std::io::Error::new(std::io::ErrorKind::Other, "io")),
        ];
        for err in errors {
            let result = serde_json::to_string(&err);
            assert!(result.is_ok(), "Failed to serialize {:?}", err);
            assert!(!result.unwrap().is_empty());
        }
    }

    // ========================================================================
    // ResultExt trait — context() and with_context()
    // ========================================================================

    #[test]
    fn test_context_on_ok_passes_through() {
        let result: std::result::Result<i32, String> = Ok(42);
        let with_ctx: Result<i32> = result.context("should not appear");
        assert_eq!(with_ctx.unwrap(), 42);
    }

    #[test]
    fn test_context_on_err_wraps_message() {
        let result: std::result::Result<i32, String> = Err("original".into());
        let with_ctx: Result<i32> = result.context("loading settings");
        let err = with_ctx.unwrap_err();
        assert_eq!(err.to_string(), "loading settings: original");
    }

    #[test]
    fn test_with_context_lazy_evaluation() {
        // On Ok, the closure should NOT be called
        let mut called = false;
        let result: std::result::Result<i32, String> = Ok(1);
        let _ = result.with_context(|| {
            called = true;
            "expensive context".into()
        });
        assert!(!called, "with_context closure should not be called on Ok");
    }

    #[test]
    fn test_with_context_on_err() {
        let result: std::result::Result<i32, String> = Err("db error".into());
        let with_ctx = result.with_context(|| format!("query for user {}", 42));
        let err = with_ctx.unwrap_err();
        assert_eq!(err.to_string(), "query for user 42: db error");
    }

    #[test]
    fn test_context_preserves_internal_variant() {
        let result: std::result::Result<(), String> = Err("fail".into());
        let err = result.context("ctx").unwrap_err();
        match err {
            FourDaError::Internal(msg) => assert!(msg.contains("ctx") && msg.contains("fail")),
            other => panic!("Expected Internal, got {:?}", other),
        }
    }

    #[test]
    fn test_context_works_with_io_error() {
        let result: std::result::Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        ));
        let err = result.context("reading config").unwrap_err();
        assert!(err.to_string().contains("reading config"));
        assert!(err.to_string().contains("denied"));
    }

    // ========================================================================
    // Question mark operator integration
    // ========================================================================

    fn parse_port(s: &str) -> Result<u16> {
        let port: u16 = s.parse().map_err(|e: std::num::ParseIntError| {
            FourDaError::Config(format!("invalid port '{}': {}", s, e))
        })?;
        Ok(port)
    }

    #[test]
    fn test_question_mark_success() {
        assert_eq!(parse_port("8080").unwrap(), 8080);
    }

    #[test]
    fn test_question_mark_error() {
        let err = parse_port("abc").unwrap_err();
        assert!(err.to_string().contains("invalid port 'abc'"));
    }

    #[test]
    fn test_question_mark_overflow() {
        let err = parse_port("99999").unwrap_err();
        assert!(err.to_string().contains("invalid port '99999'"));
    }

    // ========================================================================
    // Edge cases
    // ========================================================================

    #[test]
    fn test_empty_error_message() {
        let err = FourDaError::Internal(String::new());
        assert_eq!(err.to_string(), "");
        // Should still serialize
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"\"");
    }

    #[test]
    fn test_error_with_special_characters() {
        let err = FourDaError::Internal("error with \"quotes\" and \nnewlines".into());
        let json = serde_json::to_string(&err).unwrap();
        // JSON should properly escape special chars
        assert!(json.contains("\\\"quotes\\\""));
        assert!(json.contains("\\n"));
    }

    #[test]
    fn test_error_with_unicode() {
        let err = FourDaError::Internal("Ошибка: файл не найден 🔥".into());
        assert_eq!(err.to_string(), "Ошибка: файл не найден 🔥");
        let json = serde_json::to_string(&err).unwrap();
        assert!(!json.is_empty());
    }

    #[test]
    fn test_nested_context() {
        let result: std::result::Result<(), String> = Err("root cause".into());
        let err = result.context("layer 1").context("layer 2").unwrap_err();
        // Should show outermost context wrapping inner
        assert!(err.to_string().contains("layer 2"));
        assert!(err.to_string().contains("layer 1"));
        assert!(err.to_string().contains("root cause"));
    }

    // ========================================================================
    // Additional error-path tests
    // ========================================================================

    // --- ResultExt: .context() preserves the original error message ---

    #[test]
    fn test_context_preserves_original_error_message() {
        let original = "connection refused";
        let result: std::result::Result<(), String> = Err(original.into());
        let err = result.context("fetching feed").unwrap_err();
        // The original message must appear verbatim after the colon
        assert!(
            err.to_string().ends_with(original),
            "Expected error to end with '{}', got '{}'",
            original,
            err.to_string()
        );
    }

    // --- ResultExt: .with_context() preserves the original error message ---

    #[test]
    fn test_with_context_preserves_original_error_message() {
        let result: std::result::Result<(), String> = Err("timeout".into());
        let err = result
            .with_context(|| format!("polling source {}", "HackerNews"))
            .unwrap_err();
        assert_eq!(err.to_string(), "polling source HackerNews: timeout");
    }

    // --- with_context closure IS invoked on Err path ---

    #[test]
    fn test_with_context_closure_called_on_err() {
        let mut called = false;
        let result: std::result::Result<i32, String> = Err("fail".into());
        let _ = result.with_context(|| {
            called = true;
            "context msg".into()
        });
        assert!(called, "with_context closure must be called on Err");
    }

    // --- with_context on Err produces Internal variant ---

    #[test]
    fn test_with_context_produces_internal_variant() {
        let result: std::result::Result<(), String> = Err("oops".into());
        let err = result.with_context(|| "wrapping".into()).unwrap_err();
        match err {
            FourDaError::Internal(msg) => {
                assert!(msg.contains("wrapping"));
                assert!(msg.contains("oops"));
            }
            other => panic!("Expected Internal, got {:?}", other),
        }
    }

    // --- From<rusqlite::Error> produces Db variant ---

    #[test]
    fn test_from_rusqlite_error_creates_db() {
        let sqlite_err = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
            Some("UNIQUE constraint failed".to_string()),
        );
        let err: FourDaError = sqlite_err.into();
        match &err {
            FourDaError::Db(_) => {}
            other => panic!("Expected Db, got {:?}", other),
        }
        assert!(err.to_string().starts_with("Database error:"));
    }

    // --- Display for Db variant includes "Database error:" prefix ---

    #[test]
    fn test_db_error_display_format() {
        let sqlite_err = rusqlite::Error::QueryReturnedNoRows;
        let err: FourDaError = sqlite_err.into();
        let display = err.to_string();
        assert!(
            display.starts_with("Database error:"),
            "Db display should start with 'Database error:', got '{}'",
            display
        );
    }

    // --- Display for Io variant includes "IO error:" prefix ---

    #[test]
    fn test_io_error_display_format() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broken");
        let err: FourDaError = io_err.into();
        let display = err.to_string();
        assert!(
            display.starts_with("IO error:"),
            "Io display should start with 'IO error:', got '{}'",
            display
        );
        assert!(display.contains("pipe broken"));
    }

    // --- From<&str> produces Internal variant (not just Display check) ---

    #[test]
    fn test_from_str_ref_produces_internal_variant() {
        let err: FourDaError = "static failure".into();
        match err {
            FourDaError::Internal(msg) => assert_eq!(msg, "static failure"),
            other => panic!("Expected Internal, got {:?}", other),
        }
    }

    // --- Serialization: Db variant serializes with Display prefix ---

    #[test]
    fn test_db_error_serializes_with_prefix() {
        let sqlite_err = rusqlite::Error::QueryReturnedNoRows;
        let err: FourDaError = sqlite_err.into();
        let json = serde_json::to_string(&err).expect("serialize Db error");
        // Serialized form is the Display string wrapped in quotes
        assert!(
            json.starts_with("\"Database error:"),
            "Db serialization should start with '\"Database error:', got '{}'",
            json
        );
    }

    // --- Serialization: Io variant serializes with Display prefix ---

    #[test]
    fn test_io_error_serializes_with_prefix() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let err: FourDaError = io_err.into();
        let json = serde_json::to_string(&err).expect("serialize Io error");
        assert!(
            json.starts_with("\"IO error:"),
            "Io serialization should start with '\"IO error:', got '{}'",
            json
        );
        assert!(json.contains("no such file"));
    }

    // --- Serialization: Json variant serializes with Display prefix ---

    #[test]
    fn test_json_error_serializes_with_prefix() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err: FourDaError = json_err.into();
        let json = serde_json::to_string(&err).expect("serialize Json error");
        assert!(
            json.starts_with("\"JSON error:"),
            "Json serialization should start with '\"JSON error:', got '{}'",
            json
        );
    }

    // --- ResultExt works with rusqlite::Error ---

    #[test]
    fn test_context_works_with_rusqlite_error() {
        let result: std::result::Result<(), rusqlite::Error> =
            Err(rusqlite::Error::QueryReturnedNoRows);
        let err = result.context("looking up user settings").unwrap_err();
        // context() always produces Internal, even from rusqlite errors
        match &err {
            FourDaError::Internal(msg) => {
                assert!(msg.contains("looking up user settings"));
                assert!(msg.contains("Query returned no rows"));
            }
            other => panic!("Expected Internal, got {:?}", other),
        }
    }

    // --- ResultExt works with custom Display error types ---

    #[test]
    fn test_context_works_with_custom_display_error() {
        #[derive(Debug)]
        struct CustomError(String);
        impl std::fmt::Display for CustomError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "custom: {}", self.0)
            }
        }

        let result: std::result::Result<(), CustomError> =
            Err(CustomError("validation failed".into()));
        let err = result.context("processing item").unwrap_err();
        assert_eq!(
            err.to_string(),
            "processing item: custom: validation failed"
        );
    }

    // --- Debug impl is available and contains variant name ---

    #[test]
    fn test_error_debug_contains_variant() {
        let err = FourDaError::Config("bad key".into());
        let debug = format!("{:?}", err);
        assert!(
            debug.contains("Config"),
            "Debug output should contain variant name, got '{}'",
            debug
        );
    }

    // --- Serialization roundtrip: serialize then deserialize as plain string ---

    #[test]
    fn test_serialization_produces_plain_json_string() {
        let err = FourDaError::Analysis("model unavailable".into());
        let json = serde_json::to_value(&err).expect("to_value");
        // Must be a JSON string, not an object/array
        assert!(
            json.is_string(),
            "FourDaError should serialize as a JSON string, got {:?}",
            json
        );
        assert_eq!(json.as_str().unwrap(), "Analysis error: model unavailable");
    }

    // --- From<String> with empty string ---

    #[test]
    fn test_from_empty_string_creates_internal() {
        let err: FourDaError = String::new().into();
        match err {
            FourDaError::Internal(msg) => assert!(msg.is_empty()),
            other => panic!("Expected Internal, got {:?}", other),
        }
    }
}
