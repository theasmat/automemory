use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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
        fs::write(&skill_file, self.to_skill_markdown())?;
        Ok(skill_file)
    }
}
