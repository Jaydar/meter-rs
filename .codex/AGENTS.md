## 规则

- 代理必须使用 rtk pwsh 
- 新增功能和现有风格一致
- 使用最少的代码实现
- 缩进使用 4 个空格
- 使用 UTF-8
- 不要格式化代码
- 不要随意重构无关代码
- 修改前先阅读相关文件
- 新增功能和现有风格一致
- 单个方法禁止换行
- PowerShell 7
- 版本号只写在根 `Cargo.toml`，子 crate 使用 `version.workspace = true`。
- 只用一次的东西不用封装方法


## 命名退则

- 文件名、模块名、函数名、变量名使用 `snake_case`。
- struct、enum、trait、type 使用 `PascalCase`。
- const、static 使用 `_snake_case`, 我不喜欢全大写视力不好。
- crate 名、URL 路径、命令行参数使用 `kebab-case`。
- 数据库表名、字段名使用 `snake_case`。


@C:\Users\Jaydar\.codex\RTK.md
