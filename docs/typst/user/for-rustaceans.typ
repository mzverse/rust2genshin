#import "/lib/lib.typ": *;
#show: template;

#let title = [Rust用户指南];
#let tag = <for-rustaceans>;

#set raw(lang: "rust");

如果您本身已经会用*Rust*，则可直接看本页了解差异和注意事项

= no-core

你的项目必须是 *no-std* 的，并且 *core* 其实也不会被编译出来

*core* 仅用作*类型系统*、*lang-item* 和*原语*，以及部分*内联*或*特化*函数

*core* 中的*非内联非泛型*函数一定不可使用

= 导出复合节点

一个函数对应一个复合节点（图）

使用 `#[unsafe(no_mangle)]` 标记一个函数是导出的

导出为 `.gia` 时它保留原名并作为主要资产

主要资产必定会被导入，其它只导入被引用的资产

细节见#link("/user/export.html")[导出函数]

= 字符串

`String` 是 `&'static str` 的别名，你无法自定义地*构造*一个字符串

基本上它只能来自*字符串常量*或基本类型的 `to_string`

= 整数与浮点

只有 `i32` 和 `f32`。`isize` / `u32` / `usize` 也映射到 `i32`

其余宽度（`i8` / `i16` / `i64` / `f64`…）编译不过 —— 见#link("/user/unsupported.html")[不被支持的特性]

= 递归

复合节点的本质是运行时展开，因此函数不能递归

要反复执行就用协程 —— 见#link("/user/async.html")[异步]

= 可储存类型

有些类型是不可储存的，它们的值必须被立即使用；即使存进了变量，也必须被优化成立即使用

不能以任何方式真正储存它们，包括局部变量、堆、结构体成员、列表成员

尤其是 `&mut T` 或 `PartialMut<T>`：需要存起来时改用
#link(<user-partial-mut>)[`PartialMut<Box<T>>`]

```rust
pub fn exam() {
    let mut i: i32 = 114;
    let m = &mut i;
    *m = 514;
}
```

这里 `m` 的类型是 `&mut i32`，不可储存 —— 但变量 `m` 会被优化掉，所以没问题

== 何时会被优化

如果你一眼就看出来它能被优化，那它大概就可以

判据是「值必须被立即使用，中间不跨控制流」。这条规则的实现细节在
#link("/arch/ir/index.html")[IR]页，但写代码时不需要记它 —— 编不过时把引用换成
`PartialMut<Box<T>>` 即可。
