//! Cross-project technology convergence analysis.
//!
//! Identifies shared and unique technologies across all ACE-detected projects.
//! Powers the Cross-Project Intelligence view with convergence scoring,
//! bus factor analysis, and project health comparison.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TechConvergenceReport {
    pub total_projects: usize,
    pub shared_technologies: Vec<SharedTech>,
    pub unique_technologies: Vec<UniqueTech>,
    pub convergence_score: f32,
    pub adoption_trends: Vec<AdoptionTrend>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SharedTech {
    pub name: String,
    pub category: String,
    pub project_count: usize,
    pub total_projects: usize,
    pub adoption_pct: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UniqueTech {
    pub name: String,
    pub category: String,
    pub project_path: String,
    pub bus_factor_risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AdoptionTrend {
    pub name: String,
    pub direction: String,
    pub project_count_current: usize,
    pub project_count_previous: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProjectHealthComparison {
    pub projects: Vec<ProjectHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProjectHealth {
    pub project_path: String,
    pub project_name: String,
    pub dependency_count: usize,
    pub dev_dependency_count: usize,
    pub freshness_score: f32,
    pub vulnerability_count: usize,
    pub ecosystems: Vec<String>,
}

// ============================================================================
// Core Functions
// ============================================================================

/// Analyze technology convergence across all projects.
/// Input: map of project_path -> Vec<(tech_name, tech_category)>.
pub(crate) fn analyze_convergence(
    projects: &HashMap<String, Vec<(String, String)>>,
) -> TechConvergenceReport {
    let total_projects = projects.len();
    if total_projects == 0 {
        return TechConvergenceReport {
            total_projects: 0,
            shared_technologies: vec![],
            unique_technologies: vec![],
            convergence_score: 0.0,
            adoption_trends: vec![],
        };
    }

    // Count tech occurrences across projects
    let mut tech_projects: HashMap<(String, String), Vec<String>> = HashMap::new();
    for (project_path, techs) in projects {
        for (name, category) in techs {
            tech_projects
                .entry((name.clone(), category.clone()))
                .or_default()
                .push(project_path.clone());
        }
    }

    let mut shared = Vec::new();
    let mut unique = Vec::new();

    for ((name, category), paths) in &tech_projects {
        if paths.len() > 1 {
            shared.push(SharedTech {
                name: name.clone(),
                category: category.clone(),
                project_count: paths.len(),
                total_projects,
                adoption_pct: paths.len() as f32 / total_projects as f32,
            });
        } else {
            let bus_factor = if category == "language" || category == "framework" {
                "high"
            } else {
                "medium"
            };
            unique.push(UniqueTech {
                name: name.clone(),
                category: category.clone(),
                project_path: paths[0].clone(),
                bus_factor_risk: bus_factor.to_string(),
            });
        }
    }

    // Sort shared by adoption (most shared first)
    shared.sort_by(|a, b| b.project_count.cmp(&a.project_count));

    // Convergence score: ratio of shared tech instances to total
    let total_instances: usize = tech_projects.values().map(|v| v.len()).sum();
    let shared_instances: usize = tech_projects
        .values()
        .filter(|v| v.len() > 1)
        .map(|v| v.len())
        .sum();
    let convergence_score = if total_instances > 0 {
        shared_instances as f32 / total_instances as f32
    } else {
        0.0
    };

    TechConvergenceReport {
        total_projects,
        shared_technologies: shared,
        unique_technologies: unique,
        convergence_score,
        adoption_trends: vec![],
    }
}

/// Compare health metrics across projects.
pub(crate) fn compare_project_health(
    projects: Vec<(String, usize, usize, f32, usize, Vec<String>)>,
) -> ProjectHealthComparison {
    let mut health_list: Vec<ProjectHealth> = projects
        .into_iter()
        .map(|(path, deps, dev_deps, fresh, vulns, ecos)| {
            let name = std::path::Path::new(&path)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&path)
                .to_string();
            ProjectHealth {
                project_path: path,
                project_name: name,
                dependency_count: deps,
                dev_dependency_count: dev_deps,
                freshness_score: fresh,
                vulnerability_count: vulns,
                ecosystems: ecos,
            }
        })
        .collect();

    // Sort by freshness (healthiest first)
    health_list.sort_by(|a, b| {
        b.freshness_score
            .partial_cmp(&a.freshness_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    ProjectHealthComparison {
        projects: health_list,
    }
}

/// Find dependencies shared across multiple projects.
pub(crate) fn find_cross_project_deps(
    project_deps: &HashMap<String, Vec<(String, String)>>, // project -> [(dep_name, ecosystem)]
) -> Vec<(String, String, Vec<String>)> {
    // (dep_name, ecosystem, [project_paths])
    let mut dep_projects: HashMap<(String, String), Vec<String>> = HashMap::new();

    for (project, deps) in project_deps {
        for (name, eco) in deps {
            dep_projects
                .entry((name.clone(), eco.clone()))
                .or_default()
                .push(project.clone());
        }
    }

    let mut cross: Vec<_> = dep_projects
        .into_iter()
        .filter(|(_, projects)| projects.len() > 1)
        .map(|((name, eco), projects)| (name, eco, projects))
        .collect();

    cross.sort_by(|a, b| b.2.len().cmp(&a.2.len()));
    cross
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_tech_convergence() -> crate::error::Result<serde_json::Value> {
    // Build project->tech map from ACE-detected tech across all context dirs
    let conn = crate::open_db_connection()?;

    let mut stmt = conn.prepare(
        "SELECT DISTINCT project_root, tech_name, tech_category \
         FROM ace_detected_tech WHERE project_root IS NOT NULL",
    )?;

    let mut projects: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    for row in rows {
        if let Ok((project, name, category)) = row {
            projects.entry(project).or_default().push((name, category));
        }
    }

    let report = analyze_convergence(&projects);
    Ok(serde_json::to_value(report)?)
}

#[tauri::command]
pub fn get_project_health_comparison() -> crate::error::Result<serde_json::Value> {
    let conn = crate::open_db_connection()?;

    let mut stmt = conn.prepare(
        "SELECT project_root, \
                COUNT(*) as dep_count, \
                SUM(CASE WHEN is_dev = 1 THEN 1 ELSE 0 END) as dev_deps \
         FROM ace_detected_tech \
         WHERE project_root IS NOT NULL \
         GROUP BY project_root",
    )?;

    let mut project_data: Vec<(String, usize, usize, f32, usize, Vec<String>)> = Vec::new();
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, usize>(1)?,
            row.get::<_, usize>(2)?,
        ))
    })?;

    for row in rows {
        if let Ok((path, deps, dev_deps)) = row {
            project_data.push((path, deps, dev_deps, 0.8, 0, vec![]));
        }
    }

    let comparison = compare_project_health(project_data);
    Ok(serde_json::to_value(comparison)?)
}

#[tauri::command]
pub fn get_cross_project_dependencies() -> crate::error::Result<serde_json::Value> {
    let conn = crate::open_db_connection()?;

    let mut stmt = conn.prepare(
        "SELECT DISTINCT project_root, tech_name, tech_category \
         FROM ace_detected_tech WHERE project_root IS NOT NULL",
    )?;

    let mut project_deps: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    for row in rows {
        if let Ok((project, name, eco)) = row {
            project_deps.entry(project).or_default().push((name, eco));
        }
    }

    let cross_deps = find_cross_project_deps(&project_deps);
    let result: Vec<serde_json::Value> = cross_deps
        .into_iter()
        .map(|(name, eco, projects)| {
            serde_json::json!({
                "name": name,
                "ecosystem": eco,
                "projects": projects,
                "project_count": projects.len(),
            })
        })
        .collect();

    Ok(serde_json::to_value(result)?)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convergence_empty() {
        let report = analyze_convergence(&HashMap::new());
        assert_eq!(report.total_projects, 0);
        assert_eq!(report.convergence_score, 0.0);
    }

    #[test]
    fn test_convergence_with_shared() {
        let mut projects = HashMap::new();
        projects.insert(
            "project-a".to_string(),
            vec![
                ("React".into(), "framework".into()),
                ("TypeScript".into(), "language".into()),
            ],
        );
        projects.insert(
            "project-b".to_string(),
            vec![
                ("React".into(), "framework".into()),
                ("Python".into(), "language".into()),
            ],
        );

        let report = analyze_convergence(&projects);
        assert_eq!(report.total_projects, 2);
        assert_eq!(report.shared_technologies.len(), 1);
        assert_eq!(report.shared_technologies[0].name, "React");
        assert_eq!(report.unique_technologies.len(), 2);
        assert!(report.convergence_score > 0.0);
    }

    #[test]
    fn test_convergence_all_unique() {
        let mut projects = HashMap::new();
        projects.insert("a".into(), vec![("Rust".into(), "language".into())]);
        projects.insert("b".into(), vec![("Python".into(), "language".into())]);

        let report = analyze_convergence(&projects);
        assert_eq!(report.shared_technologies.len(), 0);
        assert_eq!(report.unique_technologies.len(), 2);
        assert_eq!(report.convergence_score, 0.0);
    }

    #[test]
    fn test_project_health_sorting() {
        let projects = vec![
            ("bad-project".into(), 20, 8, 0.40, 3, vec!["npm".into()]),
            ("good-project".into(), 10, 5, 0.95, 0, vec!["cargo".into()]),
        ];
        let comparison = compare_project_health(projects);
        assert_eq!(comparison.projects[0].project_name, "good-project");
        assert_eq!(comparison.projects[1].project_name, "bad-project");
    }

    #[test]
    fn test_cross_project_deps() {
        let mut deps = HashMap::new();
        deps.insert(
            "a".into(),
            vec![
                ("lodash".into(), "npm".into()),
                ("react".into(), "npm".into()),
            ],
        );
        deps.insert(
            "b".into(),
            vec![
                ("lodash".into(), "npm".into()),
                ("express".into(), "npm".into()),
            ],
        );

        let cross = find_cross_project_deps(&deps);
        assert_eq!(cross.len(), 1);
        assert_eq!(cross[0].0, "lodash");
        assert_eq!(cross[0].2.len(), 2);
    }
}
