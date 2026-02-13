//! SQL Compile-Time Checker (Standalone for integration tests)
//!
//! Validates SQL queries against the database schema at test time.
//! Catches typos in column names, missing tables, and parameter mismatches.

#![allow(dead_code)]

pub mod schema_parser;
pub mod sql_extractor;
pub mod sql_validator;
pub mod vec0_support;
