import { h, type VNodeChild } from "vue";
import { useDialog } from "naive-ui";

/**
 * 统一确认弹窗。
 *
 * 全应用只保留这一种确认框形态，结构固定为三段：
 *   1. description —— 一句话说明「接下来会发生什么」
 *   2. target      —— 被操作的对象（名称用等宽字体标出，避免认错行）
 *   3. impacts     —— 影响要点，回答「为什么需要谨慎」
 *
 * 设计取舍：
 * - 图标一律用 warning（三角感叹号）。naive-ui 的 error 图标是叉号，
 *   读起来像「已经出错了」，而这里要表达的是「即将发生的风险」；
 *   真正的危险程度由确认按钮的颜色（destructive）区分。
 * - onConfirm 执行期间正按钮自动进入 loading 并禁用，防止重复提交。
 *   返回 Promise 时 naive-ui 会等它 resolve 才关闭弹窗
 *   （见 naive-ui/es/dialog/src/DialogEnvironment.mjs 的 handlePositiveClick）。
 * - 失败语义：错误交给 onError 提示，弹窗照常关闭——与既有行为一致，
 *   操作结果由消息提示承载，不让用户再手动关一次已过期的确认框。
 */
export interface ConfirmOptions {
  /** 标题写「动作本身」，如「删除渠道」，不写「确认删除」 */
  title: string;
  /** 一句话说明后果 */
  description: string;
  /** 对象类型标签，如「渠道」「预设」「模型」 */
  targetLabel?: string;
  /** 对象名称，会以等宽字体高亮 */
  target?: string;
  /** 影响要点，逐条列出 */
  impacts?: string[];
  /** 确认按钮文案，动词开头，如「删除」「恢复默认」 */
  positiveText: string;
  negativeText?: string;
  /** 破坏性操作：确认按钮标红 */
  destructive?: boolean;
  onConfirm: () => void | Promise<void>;
  /** 失败回调，通常传入 message.error */
  onError?: (error: unknown) => void;
}

function buildContent(options: ConfirmOptions): () => VNodeChild {
  return () => {
    const nodes: VNodeChild[] = [
      h("p", { class: "cf-desc" }, options.description),
    ];

    if (options.target) {
      nodes.push(
        h("div", { class: "cf-target" }, [
          options.targetLabel
            ? h("span", { class: "cf-target-label" }, options.targetLabel)
            : null,
          h("span", { class: "cf-target-name" }, options.target),
        ]),
      );
    }

    if (options.impacts?.length) {
      nodes.push(
        h(
          "ul",
          { class: "cf-impacts" },
          options.impacts.map((text, index) =>
            h("li", { class: "cf-impact", key: index }, text),
          ),
        ),
      );
    }

    return h("div", { class: "cf" }, nodes);
  };
}

export function useConfirm() {
  const dialog = useDialog();

  function confirm(options: ConfirmOptions): void {
    const instance = dialog.warning({
      class: options.destructive ? "cf-dialog is-danger" : "cf-dialog",
      // 宽度走内联样式：naive-ui 的 .n-dialog 默认宽度是运行时注入的 CSS，
      // 与 .cf-dialog 同为单类选择器，靠层叠顺序决定胜负并不可靠。
      style: { width: "424px", maxWidth: "calc(100vw - 32px)" },
      title: options.title,
      content: buildContent(options),
      positiveText: options.positiveText,
      negativeText: options.negativeText ?? "取消",
      positiveButtonProps: {
        type: options.destructive ? "error" : "primary",
      },
      onPositiveClick: async () => {
        instance.loading = true;
        try {
          await options.onConfirm();
        } catch (error) {
          options.onError?.(error);
        } finally {
          instance.loading = false;
        }
      },
    });
  }

  return { confirm };
}
