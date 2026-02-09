use std::collections::{HashMap, HashSet, VecDeque};

use qexed_wasm_runtime::config::Plugin;

#[derive(Debug)]
pub struct PluginDependencyAnalyzer<'a> {
    plugins: &'a [Plugin],
    dependency_graph: HashMap<String, HashSet<String>>,
    plugin_index: HashMap<String, usize>, // 名称到索引的映射
}

impl<'a> PluginDependencyAnalyzer<'a> {
    pub fn new(plugins: &'a [Plugin]) -> Self {
        let plugin_index: HashMap<_, _> = plugins.iter()
            .enumerate()
            .map(|(i, plugin)| (plugin.name.clone(), i))
            .collect();
        
        let mut analyzer = Self {
            plugins,
            dependency_graph: HashMap::new(),
            plugin_index,
        };
        
        analyzer.build_dependency_graph();
        analyzer
    }
    
    fn build_dependency_graph(&mut self) {
        // 初始化邻接表
        for plugin in self.plugins {
            self.dependency_graph.insert(plugin.name.clone(), HashSet::new());
        }
        
        // 构建依赖关系，只考虑实际存在的插件
        for plugin in self.plugins {
            // 处理硬依赖（depend）
            for depend_name in plugin.depend.keys() {
                if self.plugin_index.contains_key(depend_name) {
                    self.dependency_graph.get_mut(&plugin.name)
                        .unwrap()
                        .insert(depend_name.clone());
                }
            }
            
            // 处理loadbefore（反向依赖）
            for load_before_name in plugin.loadbefore.keys() {
                if self.plugin_index.contains_key(load_before_name) {
                    self.dependency_graph.get_mut(load_before_name)
                        .unwrap()
                        .insert(plugin.name.clone());
                }
            }
        }
    }
    
    /// 检测所有循环依赖路径
    pub fn detect_all_cycles(&self) -> Vec<Vec<String>> {
        let mut visited = HashSet::new();
        let mut recursion_stack = HashSet::new();
        let mut cycles = Vec::new();
        
        for plugin_name in self.dependency_graph.keys() {
            if !visited.contains(plugin_name) {
                let mut path = Vec::new();
                self.dfs_detect_cycles(
                    plugin_name, 
                    &mut visited, 
                    &mut recursion_stack, 
                    &mut path, 
                    &mut cycles
                );
            }
        }
        
        cycles
    }
    
    fn dfs_detect_cycles(
        &self,
        current: &str,
        visited: &mut HashSet<String>,
        recursion_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>
    ) {
        visited.insert(current.to_string());
        recursion_stack.insert(current.to_string());
        path.push(current.to_string());
        
        if let Some(dependencies) = self.dependency_graph.get(current) {
            for neighbor in dependencies {
                if !visited.contains(neighbor) {
                    self.dfs_detect_cycles(neighbor, visited, recursion_stack, path, cycles);
                } else if recursion_stack.contains(neighbor) {
                    // 找到循环依赖
                    if let Some(start_index) = path.iter().position(|x| x == neighbor) {
                        let cycle = path[start_index..].to_vec();
                        if cycle.len() > 1 {
                            cycles.push(cycle);
                        }
                    }
                }
            }
        }
        
        path.pop();
        recursion_stack.remove(current);
    }
    
    /// 使用Kahn算法进行拓扑排序检测循环依赖
    pub fn has_cycles(&self) -> bool {
        let mut in_degree = HashMap::new();
        let mut queue = VecDeque::new();
        
        // 计算入度
        for plugin_name in self.dependency_graph.keys() {
            in_degree.insert(plugin_name.clone(), 0);
        }
        
        for dependencies in self.dependency_graph.values() {
            for dep in dependencies {
                *in_degree.get_mut(dep).unwrap() += 1;
            }
        }
        
        // 入度为0的节点入队
        for (plugin_name, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(plugin_name.clone());
            }
        }
        
        let mut count = 0;
        while let Some(current) = queue.pop_front() {
            count += 1;
            
            if let Some(dependencies) = self.dependency_graph.get(&current) {
                for neighbor in dependencies {
                    let degree = in_degree.get_mut(neighbor).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }
        
        count != self.dependency_graph.len()
    }
    
    /// 获取参与循环依赖的所有插件名称
    pub fn get_cyclic_plugin_names(&self) -> HashSet<String> {
        let cycles = self.detect_all_cycles();
        let mut cyclic_plugins = HashSet::new();
        
        for cycle in cycles {
            for plugin_name in cycle {
                cyclic_plugins.insert(plugin_name);
            }
        }
        
        cyclic_plugins
    }
    
    /// 获取参与循环依赖的插件索引
    pub fn get_cyclic_plugin_indices(&self) -> HashSet<usize> {
        let cyclic_names = self.get_cyclic_plugin_names();
        cyclic_names.iter()
            .filter_map(|name| self.plugin_index.get(name))
            .cloned()
            .collect()
    }
    
    /// 生成依赖关系报告（用于调试）
    pub fn generate_dependency_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== 插件依赖关系分析报告 ===\n");
        report.push_str(&format!("分析插件数量: {}\n", self.plugins.len()));
        
        let cycles = self.detect_all_cycles();
        if cycles.is_empty() {
            report.push_str("✅ 未检测到循环依赖\n");
        } else {
            report.push_str(&format!("❌ 检测到 {} 个循环依赖路径：\n", cycles.len()));
            for (i, cycle) in cycles.iter().enumerate() {
                report.push_str(&format!("  {}. {}\n", i + 1, cycle.join(" → ")));
            }
        }
        
        report.push_str("\n🔗 详细依赖关系：\n");
        for (plugin, dependencies) in &self.dependency_graph {
            if !dependencies.is_empty() {
                report.push_str(&format!("  {} → [{}]\n", 
                    plugin, 
                    dependencies.iter().cloned().collect::<Vec<_>>().join(", ")));
            }
        }
        
        report
    }
}

#[derive(Debug)]
pub struct PluginCleaningResult {
    pub removed_plugins: Vec<String>,
    pub cycles_detected: Vec<Vec<String>>,
    pub remaining_count: usize,
}

impl PluginCleaningResult {
    pub fn new(removed: Vec<String>, cycles: Vec<Vec<String>>, remaining: usize) -> Self {
        Self {
            removed_plugins: removed,
            cycles_detected: cycles,
            remaining_count: remaining,
        }
    }
    
}

/// 主要的过滤函数 - 直接修改传入的插件列表
pub fn filter_cyclic_plugins(plugins: &mut Vec<Plugin>) -> PluginCleaningResult {    
    // 创建分析器（使用不可变引用）
    let analyzer = PluginDependencyAnalyzer::new(plugins);
    let cycles = analyzer.detect_all_cycles();
    
    // 获取涉及循环的插件索引
    let cyclic_indices = analyzer.get_cyclic_plugin_indices();
    let removed_plugins: Vec<String> = cyclic_indices.iter()
        .filter_map(|&idx| plugins.get(idx).map(|p| p.name.clone()))
        .collect();
    
    // 过滤掉循环依赖的插件（通过索引反向删除）
    let mut indices_to_remove: Vec<usize> = cyclic_indices.into_iter().collect();
    indices_to_remove.sort_unstable_by(|a, b| b.cmp(a)); // 从大到小排序，避免删除时索引变化
    
    for &index in &indices_to_remove {
        if index < plugins.len() {
            plugins.remove(index);
        }
    }
    
    let remaining_count = plugins.len();
    
    PluginCleaningResult::new(removed_plugins, cycles, remaining_count)
}

/// 安全加载检查（不修改原列表）
pub fn check_plugin_dependencies(plugins: &[Plugin]) -> Result<(), PluginCleaningResult> {
    let analyzer = PluginDependencyAnalyzer::new(plugins);
    
    if !analyzer.has_cycles() {
        Ok(())
    } else {
        let cycles = analyzer.detect_all_cycles();
        let cyclic_plugins = analyzer.get_cyclic_plugin_names();
        let removed_plugins: Vec<String> = cyclic_plugins.into_iter().collect();
        let remaining = plugins.len() - removed_plugins.len();
        Err(PluginCleaningResult::new(removed_plugins, cycles, remaining))
    }
}

/// 获取建议的加载顺序
pub fn get_recommended_load_order(plugins: &[Plugin]) -> Option<Vec<&Plugin>> {
    let analyzer = PluginDependencyAnalyzer::new(plugins);
    
    if analyzer.has_cycles() {
        return None;
    }
    
    let mut in_degree = HashMap::new();
    let mut queue = VecDeque::new();
    let mut result = Vec::new();
    
    // 计算入度
    for plugin_name in analyzer.dependency_graph.keys() {
        in_degree.insert(plugin_name.clone(), 0);
    }
    
    for dependencies in analyzer.dependency_graph.values() {
        for dep in dependencies {
            *in_degree.get_mut(dep).unwrap() += 1;
        }
    }
    
    // 入度为0的节点入队
    for (plugin_name, &degree) in &in_degree {
        if degree == 0 {
            queue.push_back(plugin_name.clone());
        }
    }
    
    while let Some(current) = queue.pop_front() {
        if let Some(&idx) = analyzer.plugin_index.get(&current) {
            result.push(&plugins[idx]);
        }
        
        if let Some(dependencies) = analyzer.dependency_graph.get(&current) {
            for neighbor in dependencies {
                let degree = in_degree.get_mut(neighbor).unwrap();
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(neighbor.clone());
                }
            }
        }
    }
    
    Some(result)
}

pub fn calculate_plugin_load_order(plugins: &Vec<Plugin>) -> Vec<usize> {
    if plugins.is_empty() {
        return Vec::new();
    }

    // 创建插件名称到索引的映射
    let name_to_index: HashMap<_, _> = plugins.iter()
        .enumerate()
        .map(|(idx, plugin)| (plugin.name.clone(), idx))
        .collect();

    // 构建依赖图（使用索引）
    let mut graph: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut in_degree: HashMap<usize, usize> = HashMap::new();

    // 初始化入度表
    for i in 0..plugins.len() {
        in_degree.insert(i, 0);
        graph.insert(i, Vec::new());
    }

    // 构建依赖关系
    for (current_idx, plugin) in plugins.iter().enumerate() {
        // 处理硬依赖
        for (dep_name, _) in &plugin.depend {
            if let Some(&dep_idx) = name_to_index.get(dep_name) {
                graph.entry(dep_idx)
                    .or_insert_with(Vec::new)
                    .push(current_idx);
                *in_degree.entry(current_idx).or_insert(0) += 1;
            }
        }

        // 处理软依赖（存在时才考虑）
        for (soft_dep_name, _) in &plugin.softdepend {
            if let Some(&dep_idx) = name_to_index.get(soft_dep_name) {
                graph.entry(dep_idx)
                    .or_insert_with(Vec::new)
                    .push(current_idx);
                *in_degree.entry(current_idx).or_insert(0) += 1;
            }
        }

        // 处理loadbefore关系（反向依赖）
        for (before_name, _) in &plugin.loadbefore {
            if let Some(&before_idx) = name_to_index.get(before_name) {
                graph.entry(current_idx)
                    .or_insert_with(Vec::new)
                    .push(before_idx);
                *in_degree.entry(before_idx).or_insert(0) += 1;
            }
        }
    }

    // 拓扑排序 - Kahn算法
    let mut queue: VecDeque<usize> = VecDeque::new();
    let mut result: Vec<usize> = Vec::new();

    // 找出所有入度为0的节点
    for i in 0..plugins.len() {
        if in_degree.get(&i).map_or(0, |&d| d) == 0 {
            queue.push_back(i);
        }
    }

    // 处理队列
    while let Some(current_idx) = queue.pop_front() {
        result.push(current_idx);

        if let Some(dependencies) = graph.get(&current_idx) {
            for &neighbor_idx in dependencies {
                if let Some(degree) = in_degree.get_mut(&neighbor_idx) {
                    if *degree > 0 {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(neighbor_idx);
                        }
                    }
                }
            }
        }
    }

    // 检查是否所有插件都被处理
    if result.len() != plugins.len() {
        eprintln!("警告: 可能存在循环依赖或孤立的插件");
        // 将未处理的插件添加到末尾
        let handled: HashSet<_> = result.iter().cloned().collect();
        for i in 0..plugins.len() {
            if !handled.contains(&i) {
                result.push(i);
            }
        }
    }

    result
}