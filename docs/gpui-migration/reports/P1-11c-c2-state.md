# P1-11c c2：应用状态与凭据后端显示状态

Windows，2026-11-21。Pi（`magpie/auto/deepseek-v4.1-flash`）返回候选实现，由主控审查、适配并接入。对照 `src/lib/appConfigNormalize.ts` 和 P1-11c 报告中的 TS 实跑样例。

- `app_state::normalize::normalize_app_state` 读取部分 JSON；分组键保留顺序、空串和重复项。与 JS number 不同，原生时间戳只接受可用 `i64` 精确表示的整数；拒绝小数和越界值，不作饱和转换。
- `ssh::secret_store::normalize::normalize_secret_status` 仅构造显示状态。旧 `vault` 模式复用已有映射；未知验证方式为 `none`。与 TS 的 `??` 不同，错误类型的布尔或消息字段按原生类型回退。未修改认证决策、存储或迁移路径。
- 两个适配器均为纯函数，尚未接入 P3-02。未改配置文件格式；`normalizeConfig`、默认值与类型转换不在本批内。
