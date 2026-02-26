use std::io;
use std::path::Path;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Terminal;

use crate::core::ConfigCenter;

/// 菜单面板
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPanel {
    Projects,
    Environments,
    ConfigItems,
    SharedGroup,
    ApiKeys,
    Server,
}

impl MenuPanel {
    const ALL: [MenuPanel; 6] = [
        MenuPanel::Projects,
        MenuPanel::Environments,
        MenuPanel::ConfigItems,
        MenuPanel::SharedGroup,
        MenuPanel::ApiKeys,
        MenuPanel::Server,
    ];

    fn label(self) -> &'static str {
        match self {
            MenuPanel::Projects => "Projects",
            MenuPanel::Environments => "Environments",
            MenuPanel::ConfigItems => "Config Items",
            MenuPanel::SharedGroup => "Shared Group",
            MenuPanel::ApiKeys => "API Keys",
            MenuPanel::Server => "Server",
        }
    }
}

/// 焦点区域：菜单 or 内容
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Menu,
    Content,
}

/// 内容区域的输入模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// 浏览列表
    Normal,
    /// 填写创建表单
    Creating,
    /// 确认删除
    Deleting,
}

/// TUI 应用状态
pub struct App {
    center: ConfigCenter,
    selected_menu: usize,
    focus: Focus,
    status_message: String,
    running: bool,
    // 内容区域状态
    content_items: Vec<String>,
    content_selected: usize,
    input_mode: InputMode,
    /// 当前编辑的字段索引
    input_field: usize,
    /// 表单字段：(标签, 值)
    input_fields: Vec<(String, String)>,
    /// 当前选中的项目（用于 Environments/ConfigItems/ApiKeys 面板）
    current_project: Option<String>,
    /// 当前选中的环境（用于 ConfigItems/SharedGroup 面板）
    current_env: Option<String>,
    /// API 服务器是否运行中
    server_running: bool,
}

impl App {
    /// 创建 App 实例
    pub fn new(data_path: &Path) -> crate::error::Result<Self> {
        let center = ConfigCenter::new(data_path)?;
        let mut app = Self {
            center,
            selected_menu: 0,
            focus: Focus::Menu,
            status_message: "Ready".to_string(),
            running: true,
            content_items: Vec::new(),
            content_selected: 0,
            input_mode: InputMode::Normal,
            input_field: 0,
            input_fields: Vec::new(),
            current_project: None,
            current_env: None,
            server_running: false,
        };
        app.refresh_content();
        Ok(app)
    }

    /// 从已有 ConfigCenter 创建（用于测试）
    pub fn with_center(center: ConfigCenter) -> Self {
        let mut app = Self {
            center,
            selected_menu: 0,
            focus: Focus::Menu,
            status_message: "Ready".to_string(),
            running: true,
            content_items: Vec::new(),
            content_selected: 0,
            input_mode: InputMode::Normal,
            input_field: 0,
            input_fields: Vec::new(),
            current_project: None,
            current_env: None,
            server_running: false,
        };
        app.refresh_content();
        app
    }

    pub fn center(&self) -> &ConfigCenter {
        &self.center
    }

    pub fn center_mut(&mut self) -> &mut ConfigCenter {
        &mut self.center
    }

    pub fn selected_panel(&self) -> MenuPanel {
        MenuPanel::ALL[self.selected_menu]
    }

    pub fn focus(&self) -> Focus {
        self.focus
    }

    pub fn status_message(&self) -> &str {
        &self.status_message
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn input_mode(&self) -> InputMode {
        self.input_mode
    }

    pub fn content_items(&self) -> &[String] {
        &self.content_items
    }

    pub fn content_selected(&self) -> usize {
        self.content_selected
    }

    pub fn input_fields(&self) -> &[(String, String)] {
        &self.input_fields
    }

    pub fn input_field(&self) -> usize {
        self.input_field
    }

    pub fn current_project(&self) -> Option<&str> {
        self.current_project.as_deref()
    }

    pub fn current_env(&self) -> Option<&str> {
        self.current_env.as_deref()
    }

    pub fn server_running(&self) -> bool {
        self.server_running
    }

    /// 自动选择当前项目（取第一个项目）
    fn ensure_current_project(&mut self) {
        if self.current_project.is_none() {
            if let Some(p) = self.center.list_projects().first() {
                self.current_project = Some(p.name.clone());
            }
        }
    }

    /// 自动选择当前环境（取 "default" 或第一个）
    fn ensure_current_env(&mut self) {
        self.ensure_current_project();
        if self.current_env.is_none() {
            self.current_env = Some("default".to_string());
        }
    }

    /// 确保 shared_group 中存在指定环境
    fn ensure_shared_env(&mut self, env_name: &str) {
        let exists = self
            .center
            .storage()
            .state()
            .shared_group
            .environments
            .iter()
            .any(|e| e.name == env_name);
        if !exists {
            self.center
                .storage_mut()
                .state_mut()
                .shared_group
                .environments
                .push(crate::models::Environment {
                    name: env_name.to_string(),
                    config_items: Vec::new(),
                });
            let _ = self.center.storage().save();
        }
    }

    /// 切换到下一个项目
    fn cycle_project(&mut self) {
        let projects = self.center.list_projects();
        if projects.is_empty() {
            return;
        }
        let current = self.current_project.as_deref().unwrap_or("");
        let idx = projects.iter().position(|p| p.name == current).unwrap_or(0);
        let next = (idx + 1) % projects.len();
        self.current_project = Some(projects[next].name.clone());
        // 重置环境选择
        self.current_env = Some("default".to_string());
        self.refresh_content();
        self.set_status(format!("Switched to project: {}", self.current_project.as_deref().unwrap_or("")));
    }

    /// 切换到下一个环境
    fn cycle_env(&mut self) {
        let proj = match self.current_project.as_deref() {
            Some(p) => p,
            None => return,
        };
        // 根据面板决定从哪里获取环境列表
        let env_names: Vec<String> = match self.selected_panel() {
            MenuPanel::SharedGroup => {
                // SharedGroup 使用 shared_group 的环境
                self.center
                    .storage()
                    .state()
                    .shared_group
                    .environments
                    .iter()
                    .map(|e| e.name.clone())
                    .collect()
            }
            _ => {
                // 其他面板使用项目的环境
                match self.center.list_environments(proj) {
                    Ok(envs) => envs.iter().map(|e| e.name.clone()).collect(),
                    Err(_) => return,
                }
            }
        };
        if env_names.is_empty() {
            return;
        }
        let current = self.current_env.as_deref().unwrap_or("");
        let idx = env_names.iter().position(|n| n == current).unwrap_or(0);
        let next = (idx + 1) % env_names.len();
        self.current_env = Some(env_names[next].clone());
        self.refresh_content();
        self.set_status(format!("Switched to env: {}", self.current_env.as_deref().unwrap_or("")));
    }

    /// 根据当前面板刷新内容列表
    pub fn refresh_content(&mut self) {
        self.content_items = match self.selected_panel() {
            MenuPanel::Projects => self
                .center
                .list_projects()
                .iter()
                .map(|p| {
                    if let Some(desc) = &p.description {
                        format!("{} ({})", p.name, desc)
                    } else {
                        p.name.clone()
                    }
                })
                .collect(),
            MenuPanel::Environments => {
                self.ensure_current_project();
                match self.current_project.as_deref() {
                    Some(proj) => match self.center.list_environments(proj) {
                        Ok(envs) => envs.iter().map(|e| e.name.clone()).collect(),
                        Err(_) => Vec::new(),
                    },
                    None => Vec::new(),
                }
            }
            MenuPanel::ConfigItems => {
                self.ensure_current_env();
                let proj = self.current_project.as_deref().unwrap_or("");
                let env = self.current_env.as_deref().unwrap_or("default");
                match self.center.list_config_items(proj, env) {
                    Ok(items) => items.iter().map(|c| format!("{} = {}", c.key, c.value)).collect(),
                    Err(_) => Vec::new(),
                }
            }
            MenuPanel::SharedGroup => {
                self.ensure_current_env();
                let env = self.current_env.as_deref().unwrap_or("default");
                match self.center.list_shared_items(env) {
                    Ok(items) => items.iter().map(|c| format!("{} = {}", c.key, c.value)).collect(),
                    Err(_) => Vec::new(),
                }
            }
            MenuPanel::ApiKeys => {
                self.ensure_current_project();
                match self.current_project.as_deref() {
                    Some(proj) => match self.center.list_api_keys(proj) {
                        Ok(keys) => keys.iter().map(|k| format!("{} ({})", k.key, k.project)).collect(),
                        Err(_) => Vec::new(),
                    },
                    None => Vec::new(),
                }
            }
            MenuPanel::Server => {
                if self.server_running {
                    vec!["Server: Running on :3000".to_string()]
                } else {
                    vec!["Server: Stopped".to_string()]
                }
            }
        };
        // 修正选中索引
        if self.content_items.is_empty() {
            self.content_selected = 0;
        } else if self.content_selected >= self.content_items.len() {
            self.content_selected = self.content_items.len() - 1;
        }
    }

    /// 启动 TUI 事件循环
    pub fn run(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        while self.running {
            terminal.draw(|frame| self.render(frame))?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                self.handle_key(key.code);
            }
        }
        Ok(())
    }

    /// 处理键盘输入
    fn handle_key(&mut self, code: KeyCode) {
        // 创建/删除模式下优先处理
        match self.input_mode {
            InputMode::Creating => {
                self.handle_create_key(code);
                return;
            }
            InputMode::Deleting => {
                self.handle_delete_key(code);
                return;
            }
            InputMode::Normal => {}
        }

        match code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Menu => Focus::Content,
                    Focus::Content => Focus::Menu,
                };
            }
            _ if self.focus == Focus::Menu => self.handle_menu_key(code),
            _ if self.focus == Focus::Content => self.handle_content_key(code),
            _ => {}
        }
    }

    /// 菜单区域按键处理
    fn handle_menu_key(&mut self, code: KeyCode) {
        let prev = self.selected_menu;
        match code {
            KeyCode::Up => {
                if self.selected_menu > 0 {
                    self.selected_menu -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected_menu < MenuPanel::ALL.len() - 1 {
                    self.selected_menu += 1;
                }
            }
            KeyCode::Enter => {
                let panel = self.selected_panel();
                self.set_status(format!("Selected: {}", panel.label()));
            }
            _ => {}
        }
        // 面板切换时刷新内容
        if self.selected_menu != prev {
            self.refresh_content();
        }
    }

    /// 内容区域 Normal 模式按键处理
    fn handle_content_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up => {
                if self.content_selected > 0 {
                    self.content_selected -= 1;
                }
            }
            KeyCode::Down => {
                if !self.content_items.is_empty()
                    && self.content_selected < self.content_items.len() - 1
                {
                    self.content_selected += 1;
                }
            }
            KeyCode::Char('n') => {
                self.start_creating();
            }
            KeyCode::Char('d') => {
                if !self.content_items.is_empty() {
                    self.input_mode = InputMode::Deleting;
                    self.set_status("Delete? y=confirm, n/Esc=cancel");
                }
            }
            KeyCode::Char('e') => {
                // 编辑：仅 ConfigItems 和 SharedGroup 支持
                self.start_editing();
            }
            KeyCode::Char('p') => {
                // 切换项目上下文
                self.cycle_project();
            }
            KeyCode::Char('v') => {
                // 切换环境上下文
                self.cycle_env();
            }
            KeyCode::Char('s') => {
                // Server 面板：切换服务器状态
                if self.selected_panel() == MenuPanel::Server {
                    self.server_running = !self.server_running;
                    if self.server_running {
                        self.set_status("Server started on :3000 (hint: run `cargo run -- serve` in terminal)");
                    } else {
                        self.set_status("Server stopped");
                    }
                    self.refresh_content();
                }
            }
            _ => {}
        }
    }

    /// 开始创建流程，初始化表单字段
    fn start_creating(&mut self) {
        match self.selected_panel() {
            MenuPanel::Projects => {
                self.input_fields = vec![
                    ("Name".to_string(), String::new()),
                    ("Description".to_string(), String::new()),
                ];
            }
            MenuPanel::Environments => {
                self.ensure_current_project();
                if self.current_project.is_none() {
                    self.set_status("Error: no project selected, create a project first");
                    return;
                }
                self.input_fields = vec![("Name".to_string(), String::new())];
            }
            MenuPanel::ConfigItems => {
                self.ensure_current_env();
                if self.current_project.is_none() {
                    self.set_status("Error: no project selected");
                    return;
                }
                self.input_fields = vec![
                    ("Key".to_string(), String::new()),
                    ("Value".to_string(), String::new()),
                ];
            }
            MenuPanel::SharedGroup => {
                self.ensure_current_env();
                self.input_fields = vec![
                    ("Key".to_string(), String::new()),
                    ("Value".to_string(), String::new()),
                ];
            }
            MenuPanel::ApiKeys => {
                // API Key 不需要表单，直接生成
                self.ensure_current_project();
                match self.current_project.as_deref() {
                    Some(proj) => match self.center.generate_api_key(proj) {
                        Ok(key) => {
                            self.set_status(format!("API Key generated: {}", key.key));
                            self.refresh_content();
                        }
                        Err(e) => self.set_status(format!("Error: {}", e)),
                    },
                    None => self.set_status("Error: no project selected"),
                }
                return;
            }
            MenuPanel::Server => {
                self.set_status("Use 's' to toggle server");
                return;
            }
        }
        self.input_field = 0;
        self.input_mode = InputMode::Creating;
        self.set_status("Creating... Tab=next field, Enter=confirm, Esc=cancel");
    }

    /// 开始编辑流程（仅 ConfigItems 和 SharedGroup）
    fn start_editing(&mut self) {
        if self.content_items.is_empty() {
            return;
        }
        match self.selected_panel() {
            MenuPanel::ConfigItems | MenuPanel::SharedGroup => {
                // 从 content_items 解析 "key = value"
                if let Some(item) = self.content_items.get(self.content_selected) {
                    let (key, value) = match item.split_once(" = ") {
                        Some((k, v)) => (k.to_string(), v.to_string()),
                        None => return,
                    };
                    self.input_fields = vec![
                        ("Key".to_string(), key),
                        ("Value".to_string(), value),
                    ];
                    self.input_field = 1; // 默认聚焦到 Value 字段
                    self.input_mode = InputMode::Creating;
                    self.set_status("Editing... Tab=next field, Enter=confirm, Esc=cancel");
                }
            }
            _ => {}
        }
    }

    /// 创建模式按键处理
    fn handle_create_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.input_fields.clear();
                self.set_status("Cancelled");
            }
            KeyCode::Tab | KeyCode::BackTab => {
                if !self.input_fields.is_empty() {
                    if code == KeyCode::BackTab && self.input_field > 0 {
                        self.input_field -= 1;
                    } else if code == KeyCode::Tab {
                        self.input_field = (self.input_field + 1) % self.input_fields.len();
                    }
                }
            }
            KeyCode::Enter => {
                self.confirm_create();
            }
            KeyCode::Backspace => {
                if let Some((_label, value)) = self.input_fields.get_mut(self.input_field) {
                    value.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some((_label, value)) = self.input_fields.get_mut(self.input_field) {
                    value.push(c);
                }
            }
            _ => {}
        }
    }

    /// 确认创建
    fn confirm_create(&mut self) {
        match self.selected_panel() {
            MenuPanel::Projects => {
                let name = self.field_value(0);
                let desc = self.field_value(1);
                if name.is_empty() {
                    self.set_status("Error: name cannot be empty");
                    return;
                }
                let desc_opt = if desc.is_empty() { None } else { Some(desc.as_str()) };
                match self.center.create_project(&name, desc_opt) {
                    Ok(_) => self.set_status(format!("Project '{}' created", name)),
                    Err(e) => self.set_status(format!("Error: {}", e)),
                }
            }
            MenuPanel::Environments => {
                let env_name = self.field_value(0);
                if env_name.is_empty() {
                    self.set_status("Error: name cannot be empty");
                    return;
                }
                let proj = self.current_project.clone().unwrap_or_default();
                match self.center.create_environment(&proj, &env_name) {
                    Ok(_) => self.set_status(format!("Environment '{}' created", env_name)),
                    Err(e) => self.set_status(format!("Error: {}", e)),
                }
            }
            MenuPanel::ConfigItems => {
                let key = self.field_value(0);
                let raw_value = self.field_value(1);
                if key.is_empty() {
                    self.set_status("Error: key cannot be empty");
                    return;
                }
                let json_value = Self::parse_json_value(&raw_value);
                let proj = self.current_project.clone().unwrap_or_default();
                let env = self.current_env.clone().unwrap_or_else(|| "default".to_string());
                // 尝试更新，如果不存在则创建
                match self.center.update_config_item(&proj, &env, &key, json_value.clone()) {
                    Ok(_) => self.set_status(format!("Config '{}' updated", key)),
                    Err(_) => match self.center.create_config_item(&proj, &env, &key, json_value) {
                        Ok(_) => self.set_status(format!("Config '{}' created", key)),
                        Err(e) => self.set_status(format!("Error: {}", e)),
                    },
                }
            }
            MenuPanel::SharedGroup => {
                let key = self.field_value(0);
                let raw_value = self.field_value(1);
                if key.is_empty() {
                    self.set_status("Error: key cannot be empty");
                    return;
                }
                let json_value = Self::parse_json_value(&raw_value);
                let env = self.current_env.clone().unwrap_or_else(|| "default".to_string());
                // 确保 shared_group 有该环境
                self.ensure_shared_env(&env);
                // 尝试更新，如果不存在则创建
                match self.center.update_shared_item(&env, &key, json_value.clone()) {
                    Ok(_) => self.set_status(format!("Shared config '{}' updated", key)),
                    Err(_) => match self.center.create_shared_item(&env, &key, json_value) {
                        Ok(_) => self.set_status(format!("Shared config '{}' created", key)),
                        Err(e) => self.set_status(format!("Error: {}", e)),
                    },
                }
            }
            _ => {
                self.set_status("Not supported");
            }
        }
        self.input_mode = InputMode::Normal;
        self.input_fields.clear();
        self.refresh_content();
    }

    /// 从表单字段获取 trimmed 值
    fn field_value(&self, idx: usize) -> String {
        self.input_fields
            .get(idx)
            .map(|(_, v)| v.trim().to_string())
            .unwrap_or_default()
    }

    /// 尝试将字符串解析为 JSON 值，失败则作为字符串
    fn parse_json_value(raw: &str) -> serde_json::Value {
        serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()))
    }

    /// 删除模式按键处理
    fn handle_delete_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('y') => {
                self.confirm_delete();
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.set_status("Cancelled");
            }
            _ => {}
        }
    }

    /// 确认删除
    fn confirm_delete(&mut self) {
        match self.selected_panel() {
            MenuPanel::Projects => {
                if let Some(item) = self.content_items.get(self.content_selected) {
                    let project_name = item.split(" (").next().unwrap_or(item).to_string();
                    match self.center.delete_project(&project_name) {
                        Ok(()) => self.set_status(format!("Project '{}' deleted", project_name)),
                        Err(e) => self.set_status(format!("Error: {}", e)),
                    }
                }
            }
            MenuPanel::Environments => {
                if let Some(env_name) = self.content_items.get(self.content_selected).cloned() {
                    let proj = self.current_project.clone().unwrap_or_default();
                    match self.center.delete_environment(&proj, &env_name) {
                        Ok(()) => self.set_status(format!("Environment '{}' deleted", env_name)),
                        Err(e) => self.set_status(format!("Error: {}", e)),
                    }
                }
            }
            MenuPanel::ConfigItems => {
                if let Some(item) = self.content_items.get(self.content_selected) {
                    let key = item.split(" = ").next().unwrap_or(item).to_string();
                    let proj = self.current_project.clone().unwrap_or_default();
                    let env = self.current_env.clone().unwrap_or_else(|| "default".to_string());
                    match self.center.delete_config_item(&proj, &env, &key) {
                        Ok(()) => self.set_status(format!("Config '{}' deleted", key)),
                        Err(e) => self.set_status(format!("Error: {}", e)),
                    }
                }
            }
            MenuPanel::SharedGroup => {
                if let Some(item) = self.content_items.get(self.content_selected) {
                    let key = item.split(" = ").next().unwrap_or(item).to_string();
                    let env = self.current_env.clone().unwrap_or_else(|| "default".to_string());
                    match self.center.delete_shared_item(&env, &key) {
                        Ok(()) => self.set_status(format!("Shared config '{}' deleted", key)),
                        Err(e) => self.set_status(format!("Error: {}", e)),
                    }
                }
            }
            MenuPanel::ApiKeys => {
                if let Some(item) = self.content_items.get(self.content_selected) {
                    // 格式: "uuid (project)"
                    let api_key = item.split(" (").next().unwrap_or(item).to_string();
                    match self.center.revoke_api_key(&api_key) {
                        Ok(()) => self.set_status(format!("API Key revoked: {}", api_key)),
                        Err(e) => self.set_status(format!("Error: {}", e)),
                    }
                }
            }
            MenuPanel::Server => {
                self.set_status("Use 's' to toggle server");
            }
        }
        self.input_mode = InputMode::Normal;
        self.refresh_content();
    }

    /// 渲染整个界面
    fn render(&self, frame: &mut ratatui::Frame) {
        let area = frame.area();

        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_title(frame, outer[0]);
        self.render_body(frame, outer[1]);
        self.render_status(frame, outer[2]);
    }

    fn render_title(&self, frame: &mut ratatui::Frame, area: Rect) {
        let title = Paragraph::new("Config Center - TUI Manager")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, area);
    }

    fn render_body(&self, frame: &mut ratatui::Frame, area: Rect) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(20), Constraint::Min(1)])
            .split(area);

        self.render_menu(frame, cols[0]);
        self.render_content(frame, cols[1]);
    }

    fn render_menu(&self, frame: &mut ratatui::Frame, area: Rect) {
        let items: Vec<ListItem> = MenuPanel::ALL
            .iter()
            .enumerate()
            .map(|(i, panel)| {
                let style = if i == self.selected_menu {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let prefix = if i == self.selected_menu { "> " } else { "  " };
                ListItem::new(format!("{}{}", prefix, panel.label())).style(style)
            })
            .collect();

        let border_style = if self.focus == Focus::Menu {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let menu = List::new(items).block(
            Block::default()
                .title(" Menu ")
                .borders(Borders::ALL)
                .border_style(border_style),
        );
        frame.render_widget(menu, area);
    }

    fn render_content(&self, frame: &mut ratatui::Frame, area: Rect) {
        let panel = self.selected_panel();
        let border_style = if self.focus == Focus::Content {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        // 构建标题，包含上下文信息
        let title = self.content_title(panel);
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        match self.input_mode {
            InputMode::Normal => {
                if self.content_items.is_empty() {
                    let hint = match panel {
                        MenuPanel::Server => "Press 's' to toggle server.",
                        MenuPanel::ApiKeys => "Press 'n' to generate. Press 'p' to switch project.",
                        _ => "No items. Press 'n' to create.",
                    };
                    let content = Paragraph::new(hint).block(block);
                    frame.render_widget(content, area);
                } else {
                    let items: Vec<ListItem> = self
                        .content_items
                        .iter()
                        .enumerate()
                        .map(|(i, item)| {
                            let style = if i == self.content_selected {
                                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default()
                            };
                            let prefix = if i == self.content_selected { "> " } else { "  " };
                            ListItem::new(format!("{}{}", prefix, item)).style(style)
                        })
                        .collect();
                    let list = List::new(items).block(block);
                    frame.render_widget(list, area);
                }
            }
            InputMode::Creating => {
                let mut lines: Vec<Line> = Vec::new();
                lines.push(Line::from(Span::styled(
                    format!("Create/Edit {}:", panel.label()),
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));

                for (i, (label, value)) in self.input_fields.iter().enumerate() {
                    let is_active = i == self.input_field;
                    let indicator = if is_active { "▶ " } else { "  " };
                    let label_style = if is_active {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    lines.push(Line::from(vec![
                        Span::raw(indicator),
                        Span::styled(format!("{}: ", label), label_style),
                        Span::styled(value.as_str(), Style::default().fg(Color::White)),
                        if is_active {
                            Span::styled("█", Style::default().fg(Color::Cyan))
                        } else {
                            Span::raw("")
                        },
                    ]));
                }

                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Tab=next field  Enter=confirm  Esc=cancel",
                    Style::default().fg(Color::DarkGray),
                )));

                let content = Paragraph::new(lines).block(block);
                frame.render_widget(content, area);
            }
            InputMode::Deleting => {
                let item_name = self
                    .content_items
                    .get(self.content_selected)
                    .cloned()
                    .unwrap_or_default();
                let lines = vec![
                    Line::from(Span::styled(
                        "Confirm delete?",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(format!("  {}", item_name)),
                    Line::from(""),
                    Line::from(Span::styled(
                        "y=confirm  n/Esc=cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];
                let content = Paragraph::new(lines).block(block);
                frame.render_widget(content, area);
            }
        }
    }

    /// 构建内容面板标题（含上下文信息）
    fn content_title(&self, panel: MenuPanel) -> String {
        match panel {
            MenuPanel::Projects => " Projects ".to_string(),
            MenuPanel::Environments => {
                let proj = self.current_project.as_deref().unwrap_or("none");
                format!(" Environments [project: {}] ", proj)
            }
            MenuPanel::ConfigItems => {
                let proj = self.current_project.as_deref().unwrap_or("none");
                let env = self.current_env.as_deref().unwrap_or("default");
                format!(" Config Items [{}:{}] (p=project, v=env) ", proj, env)
            }
            MenuPanel::SharedGroup => {
                let env = self.current_env.as_deref().unwrap_or("default");
                format!(" Shared Group [env: {}] (v=env) ", env)
            }
            MenuPanel::ApiKeys => {
                let proj = self.current_project.as_deref().unwrap_or("none");
                format!(" API Keys [project: {}] (p=project) ", proj)
            }
            MenuPanel::Server => " Server ".to_string(),
        }
    }

    fn render_status(&self, frame: &mut ratatui::Frame, area: Rect) {
        let status = Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&self.status_message, Style::default().fg(Color::Green)),
            Span::raw(" | "),
            Span::styled(
                "q:Quit  Tab:Switch  ↑↓:Navigate  n:New  d:Delete  e:Edit  p:Project  v:Env",
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        let bar = Paragraph::new(status).block(Block::default().borders(Borders::ALL));
        frame.render_widget(bar, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn test_app() -> App {
        let tmp = NamedTempFile::new().unwrap();
        App::new(tmp.path()).unwrap()
    }

    #[test]
    fn test_initial_state() {
        let app = test_app();
        assert_eq!(app.selected_panel(), MenuPanel::Projects);
        assert_eq!(app.focus(), Focus::Menu);
        assert_eq!(app.status_message(), "Ready");
        assert!(app.is_running());
        assert_eq!(app.input_mode(), InputMode::Normal);
        assert!(app.content_items().is_empty());
    }

    #[test]
    fn test_menu_navigation() {
        let mut app = test_app();
        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected_panel(), MenuPanel::Environments);
        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected_panel(), MenuPanel::ConfigItems);
        app.handle_key(KeyCode::Up);
        assert_eq!(app.selected_panel(), MenuPanel::Environments);
        app.handle_key(KeyCode::Up);
        app.handle_key(KeyCode::Up);
        assert_eq!(app.selected_panel(), MenuPanel::Projects);
    }

    #[test]
    fn test_menu_navigation_lower_bound() {
        let mut app = test_app();
        for _ in 0..10 {
            app.handle_key(KeyCode::Down);
        }
        assert_eq!(app.selected_panel(), MenuPanel::Server);
    }

    #[test]
    fn test_tab_switches_focus() {
        let mut app = test_app();
        assert_eq!(app.focus(), Focus::Menu);
        app.handle_key(KeyCode::Tab);
        assert_eq!(app.focus(), Focus::Content);
        app.handle_key(KeyCode::Tab);
        assert_eq!(app.focus(), Focus::Menu);
    }

    #[test]
    fn test_content_focus_ignores_menu_nav() {
        let mut app = test_app();
        app.handle_key(KeyCode::Tab);
        assert_eq!(app.focus(), Focus::Content);
        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected_panel(), MenuPanel::Projects);
    }

    #[test]
    fn test_enter_updates_status() {
        let mut app = test_app();
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.status_message(), "Selected: Environments");
    }

    #[test]
    fn test_quit() {
        let mut app = test_app();
        assert!(app.is_running());
        app.handle_key(KeyCode::Char('q'));
        assert!(!app.is_running());
    }

    #[test]
    fn test_with_center() {
        let tmp = NamedTempFile::new().unwrap();
        let center = ConfigCenter::new(tmp.path()).unwrap();
        let app = App::with_center(center);
        assert_eq!(app.selected_panel(), MenuPanel::Projects);
    }

    #[test]
    fn test_all_panels_accessible() {
        let mut app = test_app();
        let expected = [
            MenuPanel::Projects,
            MenuPanel::Environments,
            MenuPanel::ConfigItems,
            MenuPanel::SharedGroup,
            MenuPanel::ApiKeys,
            MenuPanel::Server,
        ];
        for (i, panel) in expected.iter().enumerate() {
            assert_eq!(app.selected_panel(), *panel, "panel at index {}", i);
            if i < expected.len() - 1 {
                app.handle_key(KeyCode::Down);
            }
        }
    }

    // --- 项目管理界面测试 ---

    #[test]
    fn test_create_project_via_tui() {
        let mut app = test_app();
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        assert_eq!(app.input_mode(), InputMode::Creating);
        assert_eq!(app.input_fields().len(), 2);

        for c in "my-app".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Tab);
        for c in "test desc".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Enter);

        assert_eq!(app.input_mode(), InputMode::Normal);
        assert_eq!(app.content_items().len(), 1);
        assert_eq!(app.content_items()[0], "my-app (test desc)");
        assert!(app.status_message().contains("created"));
    }

    #[test]
    fn test_create_project_empty_name() {
        let mut app = test_app();
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.input_mode(), InputMode::Creating);
        assert!(app.status_message().contains("empty"));
    }

    #[test]
    fn test_create_project_duplicate() {
        let mut app = test_app();
        app.center.create_project("dup", None).unwrap();
        app.refresh_content();

        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        for c in "dup".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Enter);

        assert_eq!(app.input_mode(), InputMode::Normal);
        assert!(app.status_message().contains("Error"));
    }

    #[test]
    fn test_create_project_cancel() {
        let mut app = test_app();
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        assert_eq!(app.input_mode(), InputMode::Creating);
        app.handle_key(KeyCode::Esc);
        assert_eq!(app.input_mode(), InputMode::Normal);
        assert!(app.content_items().is_empty());
    }

    #[test]
    fn test_delete_project_via_tui() {
        let mut app = test_app();
        app.center.create_project("to-delete", None).unwrap();
        app.refresh_content();
        assert_eq!(app.content_items().len(), 1);

        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('d'));
        assert_eq!(app.input_mode(), InputMode::Deleting);
        app.handle_key(KeyCode::Char('y'));
        assert_eq!(app.input_mode(), InputMode::Normal);
        assert!(app.content_items().is_empty());
        assert!(app.status_message().contains("deleted"));
    }

    #[test]
    fn test_delete_project_cancel() {
        let mut app = test_app();
        app.center.create_project("keep", None).unwrap();
        app.refresh_content();

        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('d'));
        assert_eq!(app.input_mode(), InputMode::Deleting);
        app.handle_key(KeyCode::Char('n'));
        assert_eq!(app.input_mode(), InputMode::Normal);
        assert_eq!(app.content_items().len(), 1);
    }

    #[test]
    fn test_delete_on_empty_list() {
        let mut app = test_app();
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('d'));
        assert_eq!(app.input_mode(), InputMode::Normal);
    }

    #[test]
    fn test_content_list_navigation() {
        let mut app = test_app();
        app.center.create_project("aaa", None).unwrap();
        app.center.create_project("bbb", None).unwrap();
        app.center.create_project("ccc", None).unwrap();
        app.refresh_content();

        app.handle_key(KeyCode::Tab);
        assert_eq!(app.content_selected(), 0);
        app.handle_key(KeyCode::Down);
        assert_eq!(app.content_selected(), 1);
        app.handle_key(KeyCode::Down);
        assert_eq!(app.content_selected(), 2);
        app.handle_key(KeyCode::Down);
        assert_eq!(app.content_selected(), 2);
        app.handle_key(KeyCode::Up);
        assert_eq!(app.content_selected(), 1);
        app.handle_key(KeyCode::Up);
        app.handle_key(KeyCode::Up);
        assert_eq!(app.content_selected(), 0);
    }

    #[test]
    fn test_menu_switch_refreshes_content() {
        let mut app = test_app();
        app.center.create_project("proj", None).unwrap();
        app.refresh_content();
        assert_eq!(app.content_items().len(), 1);

        app.handle_key(KeyCode::Down); // Environments
        // 自动选中 "proj" 作为 current_project，显示其环境
        assert!(!app.content_items().is_empty()); // 至少有 "default" 环境

        app.handle_key(KeyCode::Up); // 回到 Projects
        assert_eq!(app.content_items().len(), 1);
    }

    #[test]
    fn test_backspace_in_create_mode() {
        let mut app = test_app();
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        for c in "abc".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Backspace);
        assert_eq!(app.input_fields()[0].1, "ab");
    }

    #[test]
    fn test_create_project_no_description() {
        let mut app = test_app();
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        for c in "simple".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.content_items().len(), 1);
        assert_eq!(app.content_items()[0], "simple");
    }

    #[test]
    fn test_q_does_not_quit_in_create_mode() {
        let mut app = test_app();
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        app.handle_key(KeyCode::Char('q'));
        assert!(app.is_running());
        assert_eq!(app.input_fields()[0].1, "q");
    }

    // --- 11.3 环境管理界面测试 ---

    #[test]
    fn test_environment_panel_shows_envs() {
        let mut app = test_app();
        app.center.create_project("proj", None).unwrap();
        // 切到 Environments 面板
        app.handle_key(KeyCode::Down);
        // 应自动选中 proj，显示 default 环境
        assert_eq!(app.current_project(), Some("proj"));
        assert_eq!(app.content_items().len(), 1);
        assert_eq!(app.content_items()[0], "default");
    }

    #[test]
    fn test_create_environment_via_tui() {
        let mut app = test_app();
        app.center.create_project("proj", None).unwrap();
        app.handle_key(KeyCode::Down); // Environments
        app.handle_key(KeyCode::Tab); // Content focus
        app.handle_key(KeyCode::Char('n'));
        assert_eq!(app.input_mode(), InputMode::Creating);

        for c in "staging".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Enter);

        assert_eq!(app.input_mode(), InputMode::Normal);
        assert!(app.status_message().contains("created"));
        assert_eq!(app.content_items().len(), 2); // default + staging
    }

    #[test]
    fn test_delete_environment_via_tui() {
        let mut app = test_app();
        app.center.create_project("proj", None).unwrap();
        app.center.create_environment("proj", "staging").unwrap();
        app.handle_key(KeyCode::Down); // Environments
        app.handle_key(KeyCode::Tab);
        // 选中 staging（第二项）
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Char('d'));
        assert_eq!(app.input_mode(), InputMode::Deleting);
        app.handle_key(KeyCode::Char('y'));
        assert!(app.status_message().contains("deleted"));
        assert_eq!(app.content_items().len(), 1); // 只剩 default
    }

    #[test]
    fn test_environment_no_project() {
        let mut app = test_app();
        app.handle_key(KeyCode::Down); // Environments
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        // 没有项目，应报错
        assert!(app.status_message().contains("no project"));
    }

    // --- 11.4 配置项管理界面测试 ---

    #[test]
    fn test_config_items_panel() {
        let mut app = test_app();
        app.center.create_project("proj", None).unwrap();
        app.center
            .create_config_item("proj", "default", "db_host", serde_json::json!("localhost"))
            .unwrap();
        // 切到 ConfigItems 面板（index 2）
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected_panel(), MenuPanel::ConfigItems);
        assert_eq!(app.content_items().len(), 1);
        assert!(app.content_items()[0].contains("db_host"));
    }

    #[test]
    fn test_create_config_item_via_tui() {
        let mut app = test_app();
        app.center.create_project("proj", None).unwrap();
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Down); // ConfigItems
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        assert_eq!(app.input_mode(), InputMode::Creating);

        // 输入 key
        for c in "port".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Tab);
        // 输入 value
        for c in "8080".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Enter);

        assert_eq!(app.input_mode(), InputMode::Normal);
        assert!(app.status_message().contains("created"));
        assert_eq!(app.content_items().len(), 1);
    }

    #[test]
    fn test_delete_config_item_via_tui() {
        let mut app = test_app();
        app.center.create_project("proj", None).unwrap();
        app.center
            .create_config_item("proj", "default", "key1", serde_json::json!("val"))
            .unwrap();
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Down); // ConfigItems
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('d'));
        app.handle_key(KeyCode::Char('y'));
        assert!(app.status_message().contains("deleted"));
        assert!(app.content_items().is_empty());
    }

    #[test]
    fn test_edit_config_item_via_tui() {
        let mut app = test_app();
        app.center.create_project("proj", None).unwrap();
        app.center
            .create_config_item("proj", "default", "host", serde_json::json!("old"))
            .unwrap();
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Down); // ConfigItems
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('e'));
        assert_eq!(app.input_mode(), InputMode::Creating);
        // Key 字段应预填
        assert_eq!(app.input_fields()[0].1, "host");
        // 聚焦在 Value 字段
        assert_eq!(app.input_field(), 1);
    }

    #[test]
    fn test_cycle_project() {
        let mut app = test_app();
        app.center.create_project("aaa", None).unwrap();
        app.center.create_project("bbb", None).unwrap();
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Down); // ConfigItems
        app.handle_key(KeyCode::Tab);
        // 初始应选中 aaa
        assert_eq!(app.current_project(), Some("aaa"));
        app.handle_key(KeyCode::Char('p'));
        assert_eq!(app.current_project(), Some("bbb"));
        app.handle_key(KeyCode::Char('p'));
        assert_eq!(app.current_project(), Some("aaa")); // 循环
    }

    #[test]
    fn test_cycle_env() {
        let mut app = test_app();
        app.center.create_project("proj", None).unwrap();
        app.center.create_environment("proj", "staging").unwrap();
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Down); // ConfigItems
        app.handle_key(KeyCode::Tab);
        assert_eq!(app.current_env(), Some("default"));
        app.handle_key(KeyCode::Char('v'));
        assert_eq!(app.current_env(), Some("staging"));
        app.handle_key(KeyCode::Char('v'));
        assert_eq!(app.current_env(), Some("default")); // 循环
    }

    // --- 11.5 公共配置组管理界面测试 ---

    #[test]
    fn test_shared_group_panel() {
        let mut app = test_app();
        // 切到 SharedGroup 面板（index 3）
        for _ in 0..3 {
            app.handle_key(KeyCode::Down);
        }
        assert_eq!(app.selected_panel(), MenuPanel::SharedGroup);
        // 初始为空
        assert!(app.content_items().is_empty());
    }

    #[test]
    fn test_create_shared_item_via_tui() {
        let mut app = test_app();
        for _ in 0..3 {
            app.handle_key(KeyCode::Down);
        }
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        assert_eq!(app.input_mode(), InputMode::Creating);

        for c in "log_level".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Tab);
        for c in "info".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Enter);

        assert!(app.status_message().contains("created"));
        assert_eq!(app.content_items().len(), 1);
        assert!(app.content_items()[0].contains("log_level"));
    }

    #[test]
    fn test_delete_shared_item_via_tui() {
        let mut app = test_app();
        // 确保 shared_group 有 default 环境
        app.ensure_shared_env("default");
        app.center
            .create_shared_item("default", "key1", serde_json::json!("val"))
            .unwrap();
        for _ in 0..3 {
            app.handle_key(KeyCode::Down);
        }
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('d'));
        app.handle_key(KeyCode::Char('y'));
        assert!(app.status_message().contains("deleted"));
        assert!(app.content_items().is_empty());
    }

    // --- 11.6 API Key 管理界面测试 ---

    #[test]
    fn test_api_keys_panel() {
        let mut app = test_app();
        app.center.create_project("proj", None).unwrap();
        // 切到 ApiKeys 面板（index 4）
        for _ in 0..4 {
            app.handle_key(KeyCode::Down);
        }
        assert_eq!(app.selected_panel(), MenuPanel::ApiKeys);
        assert!(app.content_items().is_empty());
    }

    #[test]
    fn test_generate_api_key_via_tui() {
        let mut app = test_app();
        app.center.create_project("proj", None).unwrap();
        for _ in 0..4 {
            app.handle_key(KeyCode::Down);
        }
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        // API Key 直接生成，不进入 Creating 模式
        assert_eq!(app.input_mode(), InputMode::Normal);
        assert!(app.status_message().contains("generated"));
        assert_eq!(app.content_items().len(), 1);
    }

    #[test]
    fn test_revoke_api_key_via_tui() {
        let mut app = test_app();
        app.center.create_project("proj", None).unwrap();
        app.center.generate_api_key("proj").unwrap();
        for _ in 0..4 {
            app.handle_key(KeyCode::Down);
        }
        assert_eq!(app.content_items().len(), 1);
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('d'));
        app.handle_key(KeyCode::Char('y'));
        assert!(app.status_message().contains("revoked"));
        assert!(app.content_items().is_empty());
    }

    // --- 11.7 Server 控制测试 ---

    #[test]
    fn test_server_panel() {
        let mut app = test_app();
        for _ in 0..5 {
            app.handle_key(KeyCode::Down);
        }
        assert_eq!(app.selected_panel(), MenuPanel::Server);
        assert_eq!(app.content_items().len(), 1);
        assert!(app.content_items()[0].contains("Stopped"));
    }

    #[test]
    fn test_server_toggle() {
        let mut app = test_app();
        for _ in 0..5 {
            app.handle_key(KeyCode::Down);
        }
        app.handle_key(KeyCode::Tab);
        assert!(!app.server_running());

        app.handle_key(KeyCode::Char('s'));
        assert!(app.server_running());
        assert!(app.content_items()[0].contains("Running"));
        assert!(app.status_message().contains("started"));

        app.handle_key(KeyCode::Char('s'));
        assert!(!app.server_running());
        assert!(app.content_items()[0].contains("Stopped"));
        assert!(app.status_message().contains("stopped"));
    }

    // --- 11.8 操作结果反馈测试 ---

    #[test]
    fn test_success_message_on_create() {
        let mut app = test_app();
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        for c in "test".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Enter);
        assert!(app.status_message().contains("created"));
    }

    #[test]
    fn test_error_message_on_duplicate() {
        let mut app = test_app();
        app.center.create_project("dup", None).unwrap();
        app.refresh_content();
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        for c in "dup".chars() {
            app.handle_key(KeyCode::Char(c));
        }
        app.handle_key(KeyCode::Enter);
        assert!(app.status_message().contains("Error"));
    }

    #[test]
    fn test_cancel_message() {
        let mut app = test_app();
        app.handle_key(KeyCode::Tab);
        app.handle_key(KeyCode::Char('n'));
        app.handle_key(KeyCode::Esc);
        assert_eq!(app.status_message(), "Cancelled");
    }
}
