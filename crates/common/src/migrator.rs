use moon_config::{
    LanguageType, PartialInheritedTasksConfig, PartialProjectConfig, PartialWorkspaceConfig,
    PlatformType,
};
use moon_pdk::{AnyResult, VirtualPath};
use rustc_hash::FxHashMap;
use starbase_utils::yaml;

pub struct Migrator {
    pub platform: PlatformType,
    pub project_configs: FxHashMap<VirtualPath, PartialProjectConfig>,
    pub root: VirtualPath,
    pub tasks_configs: FxHashMap<VirtualPath, PartialInheritedTasksConfig>,
    pub workspace_config: Option<PartialWorkspaceConfig>,
    pub workspace_config_path: VirtualPath,
}

impl Migrator {
    pub fn new(workspace_root: &VirtualPath) -> AnyResult<Self> {
        Ok(Self {
            platform: PlatformType::Node,
            project_configs: FxHashMap::default(),
            tasks_configs: FxHashMap::default(),
            workspace_config: None,
            workspace_config_path: workspace_root.join(".moon/workspace.yml"),
            root: workspace_root.to_owned(),
        })
    }

    pub fn load_project_config(
        &mut self,
        project_source: &str,
    ) -> AnyResult<&mut PartialProjectConfig> {
        let project_config_path = self.root.join(project_source).join("moon.yml");

        if !self.project_configs.contains_key(&project_config_path) {
            if project_config_path.exists() {
                self.project_configs.insert(
                    project_config_path.clone(),
                    yaml::read_file(&project_config_path)?,
                );
            } else {
                self.project_configs.insert(
                    project_config_path.clone(),
                    PartialProjectConfig {
                        language: Some(
                            if self
                                .root
                                .join(project_source)
                                .join("tsconfig.json")
                                .exists()
                            {
                                LanguageType::TypeScript
                            } else {
                                LanguageType::JavaScript
                            },
                        ),
                        platform: Some(self.platform),
                        ..PartialProjectConfig::default()
                    },
                );
            }
        }

        Ok(self.project_configs.get_mut(&project_config_path).unwrap())
    }

    pub fn load_tasks_config(&mut self, scope: &str) -> AnyResult<&mut PartialProjectConfig> {
        let tasks_config_path = self.root.join(".moon/tasks").join(format!("{scope}.yml"));

        if !self.tasks_configs.contains_key(&tasks_config_path) {
            self.tasks_configs.insert(
                tasks_config_path.clone(),
                if tasks_config_path.exists() {
                    yaml::read_file(&tasks_config_path)?
                } else {
                    PartialInheritedTasksConfig::default()
                },
            );
        }

        Ok(self.project_configs.get_mut(&tasks_config_path).unwrap())
    }

    pub fn load_workspace_config(&mut self) -> AnyResult<&mut PartialWorkspaceConfig> {
        if self.workspace_config.is_none() {
            if self.workspace_config_path.exists() {
                self.workspace_config = Some(yaml::read_file(&self.workspace_config_path)?);
            } else {
                self.workspace_config = Some(PartialWorkspaceConfig::default());
            }
        }

        Ok(self.workspace_config.as_mut().unwrap())
    }

    pub fn save_configs(&self) -> AnyResult<()> {
        if let Some(workspace_config) = &self.workspace_config {
            yaml::write_file(&self.workspace_config_path, workspace_config)?;
        }

        for (tasks_config_path, tasks_config) in &self.tasks_configs {
            yaml::write_file(tasks_config_path, tasks_config)?;
        }

        for (project_config_path, project_config) in &self.project_configs {
            yaml::write_file(project_config_path, project_config)?;
        }

        Ok(())
    }
}
