#import "/lib/lib.typ": *;
#let title = [异步];
#show: template;

#set raw(lang: "rust");

节点图的控制流不允许成环，所以#link("/user/unsupported.html")[递归调用]用不了。要在节点图里「分多帧反复执行」，用的是*协程*：`#[asynchronous]` 把一个 `fn` 变成协程，每让出一次就是一帧。

```rust
use rust2genshin_lib::asynchronous::{Resume, async_loop, asynchronous};

#[asynchronous]
pub fn co() {
    let mut i = 0;
    #[async_loop]
    while i < 5 {
        log(i);
        i += 1;
    }
}
```

调用方拿到的不是 `()`，而是 `impl Coroutine<Yield = f32>`；对它 `.resume()` 一次，协程跑到下一个让出点。

= 需要的 feature

```rust
#![feature(coroutines)]
#![feature(coroutine_trait)]
#![feature(stmt_expr_attributes, proc_macro_hygiene)]
```

`stmt_expr_attributes` 是 `#[async_loop]` 这类标在表达式上的属性需要的。

= `#[asynchronous]`

标在 `fn` 上。它做两件事：

1. 把返回类型改写成 `impl Coroutine<Yield = f32>`，函数体包进 `#[coroutine] static move || -> <原返回类型> { ... }`
2. 把函数体里每个 `.await` 展开成「反复 `resume`、把 `Yielded` 继续 `yield` 出去、遇到 `Complete` 就取返回值」的循环

两条规则：

- *函数不能同时是 `async fn`* —— 用了 `#[asynchronous]` 就不能写 `async`，会报错
- *函数体里不能出现裸 `yield`* —— 会报 `Do not yield`。要让出得用下面两个宏

让出点的值类型固定是 `f32`。

= `#[async_loop]`

标在 `loop` / `while` / `for` 表达式上，在循环体末尾追加一次让出。于是*每轮循环 = 一帧*，循环才能成为节点图里合法的控制流。

循环不标 `#[async_loop]` 就不会让出 —— 整个循环会在一次 resume 里跑完。

= `sleep!` 和 `async_continue!`

```rust
sleep!(x);          // 让出一帧，带出 x（f32）
async_continue!();  // 让出一帧，然后立刻 continue 当前循环
```

`sleep!` 就是 `yield`，之所以要包一层宏，是因为裸 `yield` 被 `#[asynchronous]` 禁掉了。

= 调用别的协程

在 `#[asynchronous]` 函数里直接 `.await` 另一个协程，让出会被逐层转发出去：

```rust
#[asynchronous]
pub fn co2() -> f32 {
    let x = co0().await;   // 转发 co0 的每一次让出
    x + 1.
}
```

`Resume` trait（`lib/src/asynchronous.rs`）是 `.resume()` 的来源，签名是 `fn resume(&mut self) -> CoroutineState<f32, Self::Return>`。

= 让协程跑起来

在非协程的入口里，手动 `resume`：

```rust
#[unsafe(no_mangle)]
pub fn test_co() {
    let mut co = co();
    co.resume();   // 跑一帧
    co.resume();
}
```

或者交给任务系统（`lib/src/task.rs`）：

```rust
use rust2genshin_lib::task::Task;
let task: Task<f32> = Task::spawn(Box::new(co0()));
```

它对应 `task_spawn` 原生调用，由节点图自己负责后续的推进。

= 现状

异步这条链还能看出半成品的痕迹，别当稳定 API 用：

- `#[async_loop]` 的宏体里写着 `// TODO`（`lib-internal/src/lib.rs:91`）
- `async_point` / `async_jump`（`lib/src/asynchronous.rs`）的 `#[native(...)]` 属性目前是*注释掉*的，函数体只是返回 `0.` 的占位实现 —— 还没挂到真正的节点上
- `demo/src/lib.rs` 里的 `.await` 示例也是注释状态

后端侧是支持的：协程的 `CoroutineState`、resume 参数、upvar 都在 `core/src/compile/helper.rs` 和 `compile/mod.rs` 里建模了。缺的是 `async_point` / `async_jump` 这一层。
