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

# 构建所有支持的目标(宿主 + Linux x64 + Darwin arm64 + Darwin x64)
npm run build:all

# 也可以显式构建单个目标:
npm run build:win32-x64-msvc
npm run build:linux-x64-gnu
npm run build:darwin-arm64
npm run build:darwin-x64
```

## 测试

```bash
bun tests/test.ts
# → 编译 tests/fixtures/hello.typ 并打印 HTML + 元数据
```

## 发布

```bash
# 1. 为所有支持的目标构建,并生成 npm/ 下的 scoped 子包
npm run prepublishOnly

# 2. 登录(一次性)
npm login

# 3. 发布根包以及每个 scoped 子包
npx napi pre-publish
```

`napi pre-publish` 会遍历 `napi.targets` 中的每个目标(Windows x64、Linux x64、Darwin arm64、Darwin x64),发布对应的 `@isomtop/tyhtml-{triple}` 子包,然后发布根包(根包在 `optionalDependencies` 中列出所有子包)。消费方会自动获取与平台匹配的二进制。

## 许可证

MIT —— 详见 [LICENSE](./LICENSE)。