# TyHtml

基于 Rust + [napi-rs](https://napi.rs) 的原生 Node.js 插件,将 [Typst](https://typst.app) `.typ` 文件编译为 HTML 并提取元数据。

[English](./README.md) | 简体中文

## 安装

本项目推荐使用 [Bun](https://bun.sh) 作为包管理器(仓库随附 `bun.lock`,测试套件也基于 Bun 运行):

```bash
bun add @isomtop/tyhtml
```

其他包管理器同样可用 —— Bun 在解析平台相关的 `optionalDependencies` 速度更快,且原生二进制直接走 Bun 内置的 N-API shim 即可加载:

```bash
npm install @isomtop/tyhtml
# 或
pnpm add @isomtop/tyhtml
# 或
yarn add @isomtop/tyhtml
```

包内通过 npm `optionalDependencies` 预构建了以下平台的二进制:

| 平台 | 包名 |
|---|---|
| Windows x64 (MSVC) | `@isomtop/tyhtml-win32-x64-msvc` |
| Linux x64 (glibc) | `@isomtop/tyhtml-linux-x64-gnu` |
| macOS Apple Silicon (arm64) | `@isomtop/tyhtml-darwin-arm64` |
| macOS Intel (x64) | `@isomtop/tyhtml-darwin-x64` |

macOS 的二进制在 **API 层是统一的**,但每个架构各自打包为一个 npm 包 —— `npm install` 会自动为主机平台挑选正确的那个。如果你的平台不在上表中,`npm install` 仍会成功(二进制是 `optionalDependencies`),但导入模块会在运行时失败,需要从源码构建。

## 使用

原生插件只导出一个类 `TyHtml`。构造一次(这是显式的冷启动 —— 系统字体扫描以及 constructor 中 `fontPaths` 的扫描都在这里发生),之后 `compile` / `compileSync` 可以任意调用。

```ts
import { TyHtml } from '@isomtop/tyhtml'

// 构造函数 = 冷启动。如果有基础字体目录,在这里传入。
const engine = new TyHtml({
  fontPaths: ['C:/extra/fonts'],  // 仅在构造时扫描一次
})

// 异步版本 —— 在 worker 线程上运行,不会阻塞事件循环。
const result = await engine.compile('path/to/file.typ', {
  pretty: true,                  // 是否美化 HTML 输出
  bodyOnly: false,               // false = 完整 <!DOCTYPE>...<body>;true = 仅保留 body 内容
  noMetadata: false,             // 设为 true 可跳过 <meta> 标签查询(更快)
  metadataLabel: 'meta',         // 覆盖默认查询的元数据标签
  fontPaths: ['/tmp/extra'],     // 单次调用的额外字体目录,叠加在构造集合之上
})

console.log(result.html)
// → '<!DOCTYPE html><html>...'

const meta = result.metadata ? JSON.parse(result.metadata) : null
console.log(meta)
// → { title: 'Hello', tags: ['a', 'b'], ... }

// 同步版本 —— 共用同一个实例与缓存,直接在调用线程运行。
// 用于异步会与另一个同步消费者发生竞态的场景
// (例如 Vite 插件的 watch 回调)。
const syncResult = engine.compileSync('path/to/file.typ', { pretty: true })
```

完整的 API 定义见 [`index.d.ts`](./index.d.ts)(由 `src/lib.rs` 自动生成)。

## 文件依赖

`compile` 和 `compileSync` 均返回 `dependencies: string[]`：本轮编译实际访问过的
文件的绝对路径，经过排序、去重。包括主文件、传递的 import/include、图片、数据文件，
以及从已安装包中读取的文件。即使编译返回错误诊断，缺失的项目文件也会保留在列表中。

```ts
const result = await engine.compile('path/to/main.typ')
for (const path of result.dependencies) {
  watcher.add(path) // 注册到下游的文件监听器
}
```

动态计算的文件路径同样可以跟踪；未执行分支、字体、仅输出到 HTML 而未读取的 URL
不在列表中。资源路径保留符号链接；若监听器不会跟随符号链接，还需监听目标文件。
无法解析本地目录的缺失包不会导出路径。

编译成功后，用本轮列表替换旧监听集合；出现编译错误时，保留旧列表与本轮列表的并集，
避免提前停止编译导致遗漏依赖。监听修改、创建、删除和替换；对于缺失文件，
监听最近的已存在父目录。编译前抛出的异常（例如主文件不存在）不会返回依赖结果。

## 从源码构建

依赖:

- Rust 工具链(edition 2024)
- Node.js ≥ 14
- Linux x64 交叉编译:[zig](https://ziglang.org/) ≥ 0.13 和 `@napi-rs/cross-toolchain`(`npm i -D @napi-rs/cross-toolchain`)
- macOS(Darwin)交叉编译:Apple SDK。最简单的方式是在 macOS 上直接运行宿主构建(`npm run build` 会为当前架构生成 darwin 二进制),或按 `@napi-rs/cross-toolchain` 的文档配置 `osxcross`

```bash
# 安装 JS 依赖
npm install

# 为当前主机平台构建
npm run build

# 也可以显式构建单个目标(主机上需要对应的交叉工具链):
npm run build:win32-x64-msvc
npm run build:linux-x64-gnu
npm run build:darwin-arm64
npm run build:darwin-x64
```

## 测试

```bash
bun run test
bun run typecheck
```

## 发布

```bash
# 1. 在匹配的主机或 CI runner 上构建所有支持的目标
# 2. 将生成的 .node 文件放入 npm/<triple>/
bun run artifacts

# 3. 登录（Trusted Publisher workflow 不需要手动登录）
npm login

# 4. 依次发布 npm/<triple> 下的平台子包，最后发布根包
# 对四个平台目录分别执行下面的子 shell 命令
(cd npm/linux-x64-gnu && npm publish)
npm publish
```

常规发布路径是由匹配 `v*` tag 触发的 `publish.yml` Trusted Publisher workflow。它会构建四个平台目标、校验 tag 与 `package.json` 版本、放置全部二进制文件，并先发布平台子包，最后发布根包。

## 许可证

MIT —— 详见 [LICENSE](./LICENSE)。
