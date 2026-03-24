//! Automation Auditor Sun -- measures automation coverage in projects (daily).

use super::SunResult;

pub fn execute() -> SunResult {
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(e) => {
            return SunResult {
                success: false,
                message: format!("DB unavailable: {e}"),
                data: None,
            }
        }
    };

    let automation_patterns = [
        "Makefile",
        "Dockerfile",
        ".github/workflows",
        ".gitlab-ci",
        "Jenkinsfile",
        "docker-compose",
        ".circleci",
        "Taskfile",
        "justfile",
        "package.json",
    ];

    let mut found_patterns: Vec<String> = Vec::new();

    // Query detected_tech for automation tooling
    let automation_tech: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT name) FROM detected_tech
             WHERE LOWER(name) IN (
                 'docker', 'github-actions', 'gitlab-ci', 'jenkins',
                 'make', 'terraform', 'ansible', 'kubernetes'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if automation_tech > 0 {
        // Get the actual names
        if let Ok(mut stmt) = conn.prepare(
            "SELECT DISTINCT name FROM detected_tech
             WHERE LOWER(name) IN (
                 'docker', 'github-actions', 'gitlab-ci', 'jenkins',
                 'make', 'terraform', 'ansible', 'kubernetes'
             )",
        ) {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for row in rows.flatten() {
                    found_patterns.push(row);
                }
            }
        }
    }

    // Check context directories for automation files
    let context_dirs = crate::get_context_dirs();
    let mut ci_config_count = 0;

    for dir in context_dirs.iter().take(10) {
        for pattern in &automation_patterns[..9] {
            let check_path = dir.join(pattern);
            if check_path.exists() {
                ci_config_count += 1;
                let pat_str = pattern.to_string();
                if !found_patterns.contains(&pat_str) {
                    found_patterns.push(pat_str);
                }
            }
        }
    }

    let coverage_score =
        ((found_patterns.len() as f32 / automation_patterns.len() as f32) * 100.0).min(100.0);
    let coverage_label = if coverage_score >= 60.0 {
        "strong"
    } else if coverage_score >= 30.0 {
        "moderate"
    } else {
        "minimal"
    };

    SunResult {
        success: true,
        message: format!(
            "Automation coverage: {} ({} tools, {} CI configs found)",
            coverage_label,
            found_patterns.len(),
            ci_config_count
        ),
        data: Some(serde_json::json!({
            "coverage_score": coverage_score,
            "coverage_label": coverage_label,
            "found_patterns": found_patterns,
            "ci_config_count": ci_config_count,
            "automation_tech_count": automation_tech,
        })),
    }
}
