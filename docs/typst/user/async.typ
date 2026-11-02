#import "/lib/lib.typ": *;
#let title = [异步];
#show: template;

#set raw(lang: "rust");

节点图的控制流不允许成环，所以#link("/user/unsupported.html")[递归]用不了

要「分多帧反复执行」，用的是*协程*：`#[asynchronous]`把一个`fn`变成协程，每让出一次就是一帧

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

调用方拿到的不是`()`，而是`impl Coroutine<Yield = f32>`，`.resume()`一次跑到下一个让出点

= 需要的 feature

```rust
#![feature(coroutines)]
#![feature(coroutine_trait)]
#![feature(stmt_expr_attributes, proc_macro_hygiene)]
```

`stmt_expr_attributes`是`#[async_loop]`这类标在表达式上的属性需要的

= `#[asynchronous]`

标在`fn`上，做两件事：

1. 把返回类型改写成`impl Coroutine<Yield = f32>`，函数体包进`#[coroutine] static move || -> <原返回类型> { ... }`
2. 把每个`.await`展开成「反复`resume`、把`Yielded`继续`yield`出去、遇到`Complete`就取返回值」的循环

两条规则：

- *不能同时是`async fn`* —— 会报错
- *函数体里不能出现裸`yield`* —— 会报`Do not yield`，要让出得用下面两个宏

让出点的值类型固定是`f32`

= `#[async_loop]`

标在`loop` / `while` / `for`表达式上，在循环体末尾追加一次让出

于是*每轮循环 = 一帧*，循环才能成为节点图里合法的控制流

不标就不会让出 —— 整个循环在一次resume里跑完

= `sleep!` 和 `async_continue!`

```rust
sleep!(x);          // 让出一帧，带出 x（f32）
async_continue!();  // 让出一帧，然后立刻 continue 当前循环
```

`sleep!`就是`yield`，包一层宏是因为裸`yield`被`#[asynchronous]`禁掉了

= 调用别的协程

在`#[asynchronous]`函数里直接`.await`，让出会被逐层转发

```rust
#[asynchronous]
pub fn co2() -> f32 {
    let x = co0().await;   // 转发 co0 的每一次让出
    x + 1.
}
```

= 让它跑起来

在非协程的入口里手动`resume`

```rust
#[unsafe(no_mangle)]
pub fn test_co() {
    let mut co = co();
    co.resume();   // 跑一帧
    co.resume();
}
```

或者交给任务系统，后续推进由节点图自己负责

```rust
use rust2genshin_lib::task::Task;
let task: Task<f32> = Task::spawn(Box::new(co0()));
```

= 现状

这条链还有半成品的痕迹，别当稳定API用。

`sleep!` / `async_continue!` 底下的 `async_point` / `async_jump`
（`lib/src/asynchronous.rs:12`、`:19`）的 `#[native(...)]` 目前是*注释掉*的，
函数体只是返回 `0.` 的占位 —— 还没有挂到真正的「挂起 / 恢复」节点上。

后果是：`.await` 和 `#[async_loop]` 的代码写得出来、编得过，但把它真实推进的调度
语义还没接上。后端侧是支持协程的（状态机、resume 参数、捕获变量都建模了），缺的
就是这两个函数对应的节点。
