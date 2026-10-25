#import "/lib/lib.typ": *;
#show: template;

#let title = [Mut];

#set raw(lang: "rust");

对应Rust中的`&mut T`

表示对一个局部变量节点的引用

逻辑上分为setter和getter，编译后是`(LocalSetter, PartialMut<T>)`元组

getter给的是#link(<tag-partial-mut>)[`IrKind::PartialMut`]
