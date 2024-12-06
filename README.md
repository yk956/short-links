# 简易短链接服务

一个使用 Rust + Axum 实现的轻量级短链接服务，适合小规模使用。

## 主要特性

- 短链接生成和重定向
- 基于文件的数据持久化存储
- 简洁的 Web 管理界面
- 访问统计和最后访问时间记录
- 支持配置文件管理

## 快速开始

1. 创建配置文件 `config.json`：

```json

{
"admin_token": "your_password",
"port": 3000,
"host": "0.0.0.0"
}
```


2. 运行服务：

```bash
cargo run
```

3. 访问管理界面：`http://localhost:3000/admin/`

## 技术栈

- Rust
- Axum Web 框架
- Serde JSON
- 原生 HTML/CSS/JavaScript

## 注意事项

- 适用于小规模使用（100个链接以内）
- 数据存储在本地文件中（urls.json）
- 建议在生产环境使用更安全的密码
