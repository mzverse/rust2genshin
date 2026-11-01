extern crate alloc;

use alloc::boxed::Box;
use proc_macro::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::{parse_macro_input, visit_mut, Block, Expr, ExprAwait, ExprForLoop, ExprLoop, ExprWhile, ExprYield, ForeignItemFn, Item, ItemEnum, ItemFn, ReturnType, Type};
use zyn::zyn;

/// tool attribute 的名字。
///
/// 用 `register_tool` 注册的属性编译器**不会消耗**它 —— 它原样留在 HIR 里,
/// 后端直接用 `tcx.get_all_attrs()` 读,不用回源码里抠 span。
///
/// 宏同时也会留下 `#[native(...)]` 这条属性宏 —— 它负责把函数体换成
/// `unreachable!()`(无体声明在 `impl` 块里不合法),而**信息**由 tool
/// attribute 携带。
///
/// (proc-macro crate 不能导出 `const`,所以这个常量只在本 crate 内用。)
const TOOL: &str = "rust2genshin";

/// 发一条 tool attribute。
///
/// 目标 crate 要有 `#![feature(register_tool)] #![register_tool(rust2genshin)]`
/// —— 少了它编译器会报「找不到 crate `rust2genshin`」,那个提示看不出要干什么。
/// proc macro 只能改自己展开的那段,没法把提示挪到 crate 属性上,所以这条
/// 要求写进站点「加一个原生调用」页。
fn tool(kind: &str, args: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let path: proc_macro2::TokenStream = format!("{TOOL}::{kind}").parse().unwrap();
    quote! { #[#path(#args)] }
}

/// 把函数体换成 `unreachable!()`,并挂上 tool attribute。
///
/// `kind` 决定后端怎么理解参数:
/// - `call` —— 参数是**字符串名字**,后端按名字查(`compile_native_call`)。
/// - `calc` / `exec` —— 参数是**节点 ID**,后端按 ID 合成节点;两者只差
///   有没有控制流。
fn native_fn(kind: &str, args: &proc_macro2::TokenStream, input: TokenStream) -> TokenStream {
    if let Ok(ForeignItemFn { attrs, vis, modifiers, sig, .. }) = syn::parse(input.clone()) {
        let block = TokenStream::from(quote! {
            {
                ::core::unreachable!();
            }
        });
        let item = ItemFn {
            attrs,
            vis,
            modifiers,
            sig,
            block: Box::new(parse_macro_input!(block as Block)),
        };
        let mark = tool(kind, args);
        quote! {
            #mark
            #[allow(unused_variables)]
            #[inline(never)]
            #[rustc_no_mir_inline]
            #item
        }.into()
    } else {
        // 类型上的标记:`#[native("List")]` 标在 `struct` / `extern type` 上。
        // 这两种都不是函数,宏只发 tool attribute,原样放行。
        let mark = tool("ty", args);
        let input: proc_macro2::TokenStream = input.into();
        quote! {
            #mark
            #input
        }.into()
    }
}

#[proc_macro_attribute]
pub fn native(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: proc_macro2::TokenStream = args.into();
    native_fn("call", &args, input)
}

#[proc_macro_attribute]
pub fn native_calc(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: proc_macro2::TokenStream = args.into();
    native_fn("calc", &args, input)
}

#[proc_macro_attribute]
pub fn native_exec(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: proc_macro2::TokenStream = args.into();
    native_fn("exec", &args, input)
}

#[proc_macro_attribute]
pub fn native_enum(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: proc_macro2::TokenStream = args.into();
    let input = tag(input);
    let item = parse_macro_input!(input as ItemEnum);
    let ident = item.ident.clone();
    let mark = tool("enumty", &args);
    quote! {
        #mark
        #[repr(i32)]
        #[derive(Clone, Copy, Eq)]
        #item
        impl ::core::cmp::PartialEq for #ident {
            #[inline(always)]
            fn eq(&self, other: &Self) -> bool {
                unsafe {
                    crate::native_enum_eq(*self, *other)
                }
            }
        }
    }.into()
}

#[proc_macro_attribute]
pub fn event(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: proc_macro2::TokenStream = args.into();
    let mark = tool("event", &args);
    let input: proc_macro2::TokenStream = tag(input).into();
    quote! {
        #mark
        #input
    }.into()
}

#[proc_macro_attribute]
pub fn event_listener(_args: TokenStream, input: TokenStream) -> TokenStream {
    let item = proc_macro2::TokenStream::from(tag(input));
    // 函数上加一条 tool attribute,后端据此认出入口 —— 不能靠 `#[no_mangle]`
    // 判断,普通导出函数也有它。
    let mark = tool("listener", &proc_macro2::TokenStream::new());
    quote! {
        #mark
        #[allow(unused_attributes)]
        #[unsafe(no_mangle)]
        #item
    }.into()
}

fn tag(input: TokenStream) -> TokenStream {
    input.into_iter().map(|mut x| {
        x.set_span(Span::call_site());
        x
    }).collect()
}

#[proc_macro_attribute]
pub fn async_loop(_args: TokenStream, input: TokenStream) -> TokenStream {
    // TODO
    let mut expr = parse_macro_input!(input as Expr);
    let body = match &mut expr {
        Expr::Loop(ExprLoop { body, .. }) => body,
        Expr::ForLoop(ExprForLoop { body, .. }) => body,
        Expr::While(ExprWhile { body, .. }) => body,
        _ => panic!(),
    };
    body.stmts.push(syn::parse(zyn! {
        yield ::rust2genshin_lib::asynchronous::async_jump();
    }.into_token_stream().into()).unwrap());
    expr.into_token_stream().into()
}

#[proc_macro_attribute]
pub fn asynchronous(_args: TokenStream, input: TokenStream) -> TokenStream {
    struct Visitor(Option<syn::Error>);
    impl VisitMut for Visitor {
        fn visit_expr_mut(&mut self, node: &mut Expr) {
            if let Expr::Await(ExprAwait { attrs, base, dot_token, await_token }) = node {
                *node = match syn::parse(zyn! {
                    @for (x in attrs) { {{ x }} }
                    {
                        async fn a() {
                            async {} {{ dot_token }} {{ await_token }}
                        }
                        let mut x = {{ base }};
                        loop {
                            match {
                                use ::rust2genshin_lib::asynchronous::Resume;
                                x.resume()
                            } {
                                ::core::ops::CoroutineState::Yielded(x) => yield x,
                                ::core::ops::CoroutineState::Complete(x) => break x,
                            }
                        }
                    }
                }.into_token_stream().into()) {
                    Ok(x) => x,
                    Err(x) => {
                        self.0 = x.into();
                        return;
                    }
                };
            } else {
                visit_mut::visit_expr_mut(self, node);
            }
        }
        fn visit_expr_yield_mut(&mut self, i: &mut ExprYield) {
            self.0 = syn::Error::new(i.span(), "Do not yield").into()
        }
        fn visit_item_mut(&mut self, _node: &mut Item) {
            // do nothing
        }
    }

    let ItemFn {
        attrs, vis, modifiers, mut sig, mut block
    } = parse_macro_input!(input as ItemFn);
    let mut visitor = Visitor(None);
    visitor.visit_block_mut(&mut block);
    if let Some(err) = visitor.0 {
        return err.into_compile_error().into();
    }
    if sig.asyncness.is_some() {
        return syn::Error::new(sig.span(), "fn must not be async").into_compile_error().into();
    }
    let output = match sig.output {
        ReturnType::Default => {
            let x = quote! { () }.into();
            parse_macro_input!(x as Type)
        },
        ReturnType::Type(_, x) => *x,
    };
    sig.output = ReturnType::Type(Default::default(), syn::parse(zyn! {
        impl ::core::ops::Coroutine<Yield = f32>
    }.to_token_stream().into()).unwrap());
    zyn! {
        @for (x in attrs) { {{x}} }
        {{ vis }}
        {{ modifiers.defaultness }}
        {{ sig }} {
            #[coroutine]
            static move || -> {{ output }}
            {{ block }}
        }
    }.into_token_stream().into()
}
