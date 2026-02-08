//! Test Natural Language Query System
//!
//! Run with: cargo run --bin test_nlq --release

use fourda_lib::query::{parse_simple, QueryIntent};

fn main() {
    println!("=== Natural Language Query Test ===\n");

    let test_queries = [
        "files about rust",
        "show me pdfs from last week",
        "what did I work on yesterday",
        "summarize notes on authentication",
        "compare react vs vue",
        "how many documents about testing",
        "when was I stressed about deadlines",
        "excel files from last month",
        "timeline of my work on 4da",
    ];

    for query in &test_queries {
        println!("Query: \"{}\"", query);
        let parsed = parse_simple(query);

        println!("  Intent: {:?}", parsed.intent);
        println!("  Keywords: {:?}", parsed.keywords);

        if let Some(ref tr) = parsed.time_range {
            println!("  Time Range: {:?}", tr.relative);
        }

        if let Some(ref s) = parsed.sentiment {
            println!("  Sentiment: {:?}", s);
        }

        if !parsed.file_types.is_empty() {
            println!("  File Types: {:?}", parsed.file_types);
        }

        println!("  Confidence: {:.0}%", parsed.confidence * 100.0);
        println!();
    }

    // Test intent detection
    println!("=== Intent Detection Tests ===\n");

    let intent_tests = [
        ("find files", QueryIntent::Find),
        ("summarize my notes", QueryIntent::Summarize),
        ("compare A vs B", QueryIntent::Compare),
        ("how many files", QueryIntent::Count),
        ("timeline of project", QueryIntent::Timeline),
    ];

    let mut passed = 0;
    let mut failed = 0;

    for (query, expected) in &intent_tests {
        let parsed = parse_simple(query);
        let status = if parsed.intent == *expected {
            passed += 1;
            "✓"
        } else {
            failed += 1;
            "✗"
        };
        println!(
            "{} \"{}\" → {:?} (expected {:?})",
            status, query, parsed.intent, expected
        );
    }

    println!("\n=== Results: {} passed, {} failed ===", passed, failed);
}
