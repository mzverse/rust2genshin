#import "/lib/lib.typ": *;
#show: template;

#let title = [Mut];
#let tag = <tag-mut>

#set raw(lang: "rust");

对应Rust中的`&mut T`

表示对一个局部变量节点的引用

逻辑上分为setter和getter，编译后是`(LocalSetter, PartialMut<T>)`元组

getter给的是#link(<tag-partial-mut>)[`IrKind::PartialMut`]

扁平化成两个槽，`LocalSetter` 只能设置整个变量。字段的 `&mut` 是 `FieldMut`，见#link("/dev/ir/index.html")[IR]。
