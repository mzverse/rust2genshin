#import "/lib/lib.typ": *;
#let title = [兼容性与限制];
#show: template;

#set raw(lang: "rust");

#import "/lib/card.typ": *;

能编译，但语义和普通*Rust*不同

= 运算溢出

`i32`四则运算直接映射原生节点，*不做溢出检查*

`i32::MAX + 1`回绕成`i32::MIN`，不panic

打开溢出检查也没用 —— 对应的`assert`在编译期被丢弃（日志里能看到`Ignored assert`）

要检查就自己用`checked_*` / `saturating_*`

= 整数除法

- 除数为`0` —— 结果`0`
- `i32::MIN / -1` —— 结果`0`（普通*Rust*这里会溢出panic）

两种情况都不panic，走各自的原生节点

= 扁平化局部变量

局部变量（含参数、返回值）里的*元组、闭包、事件*被递归展开成一维变量列表

因为节点图要支持多返回值 —— `fn f() -> (f32, f32)`在图上就是两个出引脚，而不是一个元组引脚

副作用：元组、闭包编译后不再是一个整体

= 内部可变性

不支持`Cell`等内部可变性

将来可能有`RcRefCell` / `RcUnsafeCell`

需要共享可变状态就用借用，见#link("/user/borrow.html")[借用]

= 可变借用

`&mut`只支持两种目标：整体借一个*标量*局部变量，或借元组的*某个元素*

借整个元组不行 —— 元组被展平，没有「整体」可借，报`Only local singleton can be ref mut`

```rust
let mut i = 1;
let r = &mut i;        // 可以

let mut v = (1, 2);
let r = &mut v.0;      // 可以：借元素
let r = &mut v;        // 不行：借整个元组
```

改元组成员本来就不需要引用 —— `pair.0 = 10;`直接赋值

#card_attention[
    引用没逃逸出去时，*MIR*优化可能先把`&mut`消掉，让整体借用元组侥幸编过

    换个写法就会崩，别依赖
]
