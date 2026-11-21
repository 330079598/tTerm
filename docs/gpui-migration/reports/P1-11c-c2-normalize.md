# P1-11c c2：部分配置归一化

Pi（`magpie/auto/deepseek-v4.1-flash`）分别生成标量和直接保留字段的候选实现，审查时按 TS 修正取整、枚举、限额与旧存储模式。共享入口 `config::app_config_normalize::normalize_config` 适用于部分 UI 配置，不调用磁盘读写，也不替换严格的配置文件读取器。类型转换错误可返回给调用者；P3-02 的订阅与保存不在本阶段。

Windows 已通过六项 pnpm 检查、Rust fmt、严格 workspace Clippy 和 workspace 测试。测试核对平台默认配置与空输入归一化后的 100 个字段；系统语言与默认更新通道取运行环境值。另重放 173 个 TS 局部配置调用，检查输入涉及的已知字段。开启 `P11C_CONFIG_REPLAY=1` 并按原 oracle 用法运行即可重新生成 `P1-11c-evidence/config-replay.json`。三种模拟平台的重放夹具字节一致，不代表完成 macOS 或 Linux 原生验收。

直接保留的字段若不符合 Rust 类型，只回退该字段；TS 对象展开则会原样保留。回归测试显式记录字符串字号、负回滚行数、错误类型的 shell 列表和主题等差异。未知顶层字段按前一阶段决定在本机保存边界透传，不加入 `AppConfig`。
