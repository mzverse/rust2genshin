#import "/lib/lib.typ": *;
#show: template;

#let title = [SetLocal];
#let tag = <tag-set-local>;

#import "/lib/graph.typ";

#{
    import graph: *;
    node({
        "SetLocal<T>"
        ctrl_in()
        ctrl_out()
        data_in("LocalRef")[setter]
        data_in("T")[value]
    })
}

将#link(<tag-local>)[Local]设为指定值

= 为什么可以消

`SetLocal` 是执行节点，写的是「这个局部变量」。消掉它等于把写入的值挪到每一处读到
它的地方*重新求值*。这件事成立的前提只有一条：*那个值挪到哪里求值都一样*。

于是整条判据可以归成三个问题：

- 写入的值依赖谁？（纯节点随便挪，不纯的不行）
- 有哪些地方真的读到了这次写？（沿控制流回溯）
- 挪过去会不会变？（来源里有说不清的节点就拒绝）

代码在 `Optimizer::eliminate_unnecessary_local_setter`（`core/src/compile/optimize.rs:1119`）。

= 约定

- *setter* 代表可能对该变量造成影响的执行节点。暂时只考虑必定重设该变量的情况
- `&mut` 的值（`PartialMut`）被交给 `Modify`（改字段）或函数调用时，那是在*通过
  引用写*它，不是读 —— 那处 usage 不算执行点，不用沿控制流判断它读到的值是谁写的，
  直接改接到写入的值上
- 对某变量的一处使用，*executors* 指所有触发该使用的执行节点

= 执行点读到的值由谁决定

沿执行点的控制输入往回走，遇到写这个变量的 setter 就停 —— 它之后的值就是它写的，
它之前的 setter 都被它盖掉了；走到别的节点就合并各条控制输入路径的结果。

合并出多个 setter 说明有分支、两边写的值不一样，属于*不明确*；控制流成环同样不明确。

执行点自己如果是 setter，它写下的值在读取*之后*，不算「之前的 setter」。

每处 usage 因此分成三种：

- 完全取决于当前 setter → 可以改接
- 完全与当前 setter 无关（它前面总是别的 setter）→ 不动
- 不明确（有分支、有环）→ 拒绝整次优化

= 为什么阈值是 1

能存进局部变量的值（普通值、列表、结构体）最多改接*一处*；视图（`PartialMut`）和
右值直接来自执行节点的不受限制。

原因是复制：一处是「把值挪到读出它的地方」，多于是「把这个值复制 N 份」。后者等于
凭空多算 N 次，图会变大而不是变小 —— 那就不叫优化了。视图不算复制，因为视图只是
读取路径；执行节点的产物本来就该在那一次执行后重新取。

= 步骤

+ *找到所有 setter* —— `Local` 的句柄（`values_out[0]`）被谁消费了，谁就是它的 setter

+ *找出写入值来源里的 opaque 节点* —— 从 setter 的 `value in[1]` 开始 BFS（值流不会有环）：

    - 来自 extern：跳过
    - 不是 calc 节点：这是 opaque
    - 是 `Local` 节点：它所有的 setter 都是 opaque；值是 ADT（能拿到 `PartialMut`）的话，
      这个值的使用也要一起看
    - 是其它不纯的节点：拒绝优化
    - 是 pure 节点：把它的所有 `value in` 入队，继续

+ *遍历每一处 usage*：

    找到它的所有 executors；`&mut` 的值被交给 `Modify` 或函数调用时不算执行点（那是
    写），不用回溯控制流，但它的输入同样要改接到写入的值上 —— 否则这个 local 永远消不掉

+ *消除该 setter 并把值连接到有关 usages*

    被写入的值不能依赖这个变量的旧值 —— 否则改接会造环（`i += 1` 就是这种：写入值是
    `i + 1`，而 `i` 正是 usage）

= 外部

- *setter* —— 局部变量的引用不会逃逸，可忽略
- *usage* —— 视为 `LinkTarget::Export`，照常处理

= 两个连带条件

删掉最后一个 setter 时，`Local` 的句柄就没人持有了 —— `eliminate_local` 随后会把
剩下的值用户改接到「初始值」引脚上，而普通局部变量那个引脚是空的。所以这种情况下
必须*所有*值用户都能跟着改接，否则不删（`handle_would_empty`，`optimize.rs:1219`）。

自赋值（`local = local`）单独处理：删掉它不改变值，但它如果是句柄的最后一个使用者，
同样会触发上面那个断连问题。
