use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// The comment that ends every exported SKILL.md. A file without it is somebody else's and is never overwritten or removed.
pub const SKILL_MARKER: &str = "<!-- AutoMemory Verified Skill";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralSkill {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub description: String,
    pub trigger_patterns: Vec<String>,
    pub preconditions: Vec<String>,
    pub steps: Vec<String>,
    pub caveats: Vec<String>,
    pub success_count: u32,
    pub failure_count: u32,
    pub source_trace_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ProceduralSkill {
    /// Formats the procedural skill into standard Agent SKILL.md document format.
    pub fn to_skill_markdown(&self) -> String {
        let slug = self.skill_slug();

        let mut out = String::new();
        out.push_str("---\n");
        out.push_str(&format!("name: {}\n", slug));
        out.push_str(&format!(
            "description: \"{}\"\n",
            self.description.replace('"', "\\\"")
        ));
        out.push_str("version: 1.0.0\n");
        if !self.trigger_patterns.is_empty() {
            out.push_str("triggers:\n");
            for trig in &self.trigger_patterns {
                out.push_str(&format!("  - \"{}\"\n", trig.replace('"', "\\\"")));
            }
        }
        out.push_str("---\n\n");

        out.push_str(&format!("# {}\n\n", self.name));
        out.push_str(&format!("{}\n\n", self.description));

        if !self.preconditions.is_empty() {
            out.push_str("## Preconditions\n\n");
            for pre in &self.preconditions {
                out.push_str(&format!("- {}\n", pre));
            }
            out.push('\n');
        }

        out.push_str("## Procedure Steps\n\n");
        for step in &self.steps {
            out.push_str(&format!("{}\n", step));
        }
        out.push('\n');

        if !self.caveats.is_empty() {
            out.push_str("## Caveats & Failure Prevention\n\n");
            for c in &self.caveats {
                out.push_str(&format!("- ⚠️ {}\n", c));
            }
            out.push('\n');
        }

        out.push_str(&format!(
            "<!-- AutoMemory Verified Skill: {} successful run(s), 0 failures -->\n",
            self.success_count
        ));

        out
    }

    /// Derives a clean URL-friendly slug identifier for the skill directory name.
    pub fn skill_slug(&self) -> String {
        let clean = self
            .name
            .to_lowercase()
            .replace("procedural playbook:", "")
            .trim()
            .replace(|c: char| !c.is_alphanumeric() && c != '-', "-");

        let parts: Vec<&str> = clean.split('-').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            "playbook".to_string()
        } else {
            parts.join("-")
        }
    }

    /// Exports the skill to a target folder, creating <target_base>/<skill_slug>/SKILL.md
    pub fn export_to_skill_dir(&self, target_base: &Path) -> Result<PathBuf> {
        let slug = self.skill_slug();
        let skill_dir = target_base.join(&slug);
        fs::create_dir_all(&skill_dir)?;
        let skill_file = skill_dir.join("SKILL.md");
        if let Ok(existing) = fs::read_to_string(&skill_file) {
            if !existing.contains(SKILL_MARKER) {
                anyhow::bail!("{} exists and was not written by AutoMemory; kept", skill_file.display());
            }
        }
        fs::write(&skill_file, self.to_skill_markdown())?;
        Ok(skill_file)
    }
}

/// Removes the skills AutoMemory exported into `target_base` (a SKILL.md carrying [`SKILL_MARKER`]); skills written by anyone
/// else, and any other file in those folders, stay. Returns the folder names removed.
pub fn remove_exported_skills(target_base: &Path) -> Vec<String> {
    let mut removed = Vec::new();
    let Ok(entries) = fs::read_dir(target_base) else { return removed };
    for entry in entries.flatten() {
        let dir = entry.path();
        let file = dir.join("SKILL.md");
        if !fs::read_to_string(&file).is_ok_and(|t| t.contains(SKILL_MARKER)) {
            continue;
        }
        if fs::remove_file(&file).is_ok() {
            let _ = fs::remove_dir(&dir); // only when nothing else is in it
            removed.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    removed
}

#[cfg(test)]
mod export_tests {
    use super::*;

    fn skill(name: &str) -> ProceduralSkill {
        ProceduralSkill {
            id: "s1".into(),
            project_id: None,
            name: name.into(),
            description: "d".into(),
            trigger_patterns: vec![],
            preconditions: vec![],
            steps: vec!["1. do it".into()],
            caveats: vec![],
            success_count: 2,
            failure_count: 0,
            source_trace_ids: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn exports_keep_other_peoples_skills_and_removal_takes_only_ours() {
        let base = std::env::temp_dir().join(format!("am-skill-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        // a skill a person wrote under the same name, and another of theirs
        fs::create_dir_all(base.join("run-tests")).unwrap();
        fs::write(base.join("run-tests/SKILL.md"), "mine").unwrap();
        fs::create_dir_all(base.join("other")).unwrap();
        fs::write(base.join("other/SKILL.md"), "theirs").unwrap();

        assert!(skill("run tests").export_to_skill_dir(&base).is_err(), "never overwrites a person's skill");
        assert_eq!(fs::read_to_string(base.join("run-tests/SKILL.md")).unwrap(), "mine");
        let ours = skill("build release").export_to_skill_dir(&base).unwrap();
        assert!(fs::read_to_string(&ours).unwrap().contains(SKILL_MARKER));
        skill("build release").export_to_skill_dir(&base).unwrap(); // again: fine

        assert_eq!(remove_exported_skills(&base), vec!["build-release".to_string()]);
        assert!(base.join("run-tests/SKILL.md").is_file() && base.join("other/SKILL.md").is_file());
        assert!(!base.join("build-release").exists());
        let _ = fs::remove_dir_all(&base);
    }
}
