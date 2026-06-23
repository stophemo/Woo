/**
 * AI 续写补全 Tiptap 扩展。
 *
 * 在用户暂停输入约 1.5 秒后，将光标附近的文本发送到 AI 请求续写，
 * 以灰色幽灵文字（decoration）展示在光标后，按 Tab 接受补全。
 */
import { Extension } from '@tiptap/vue-3'
import { Plugin, PluginKey } from 'prosemirror-state'
import { Decoration, DecorationSet } from 'prosemirror-view'

/** 获取当前 AI 服务函数，由外部注入以避免循环依赖 */
let _getAiFn: (() => { send: (prompt: string, context: string) => Promise<string> }) | null = null

export function setAiCompleteFn(fn: () => { send: (prompt: string, context: string) => Promise<string> }) {
  _getAiFn = fn
}

export const AiComplete = Extension.create({
  name: 'aiComplete',

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey('aiComplete'),
        state: {
          init() {
            return { suggestion: '', decorationSet: DecorationSet.empty, timer: null as any }
          },
          apply(tr, prev, _oldState, newState) {
            const meta = tr.getMeta('aiComplete')
            if (meta?.type === 'set') {
              return {
                suggestion: meta.text,
                decorationSet: buildDecoration(newState, meta.text),
                timer: prev.timer,
              }
            }
            if (meta?.type === 'clear' || tr.docChanged) {
              return { suggestion: '', decorationSet: DecorationSet.empty, timer: null }
            }
            return prev
          },
        },
        props: {
          decorations(state) {
            const pluginState = this.getState(state)
            return pluginState?.decorationSet || DecorationSet.empty
          },
          handleKeyDown(view, event) {
            const pluginState = AiCompletePluginKey?.getState(view.state)
            if (event.key === 'Tab' && pluginState?.suggestion) {
              event.preventDefault()
              const { from } = view.state.selection
              const tr = view.state.tr.insertText(pluginState.suggestion, from)
              view.dispatch(tr)
              // 清除建议
              view.dispatch(view.state.tr.setMeta('aiComplete', { type: 'clear' }))
              return true
            }
            return false
          },
        },
        view(view) {
          let debounceTimer: ReturnType<typeof setTimeout> | null = null
          let lastText = ''

          function checkCompletion() {
            const state = view.state
            const { from, to } = state.selection
            if (from !== to) return // 有选区时不触发

            // 取光标前 800 字符作为上下文
            const contextStart = Math.max(0, from - 800)
            const context = state.doc.textBetween(contextStart, from, '\n\n')

            if (!context.trim() || context === lastText) return
            lastText = context

            if (!_getAiFn) return

            try {
              const { send } = _getAiFn()
              send(
                '你是一个写作助手。请根据以上文本内容，续写下一句。要求：1) 风格和语气与上文一致；2) 自然流畅，不要生硬转折；3) 只输出续写内容（不超过 30 个字），不要解释。',
                context
              ).then(suggestion => {
                if (suggestion && suggestion.trim()) {
                  view.dispatch(
                    view.state.tr.setMeta('aiComplete', {
                      type: 'set',
                      text: suggestion.trim()
                    })
                  )
                }
              }).catch(() => {
                // 静默失败
              })
            } catch {
              // 静默
            }
          }

          return {
            update(view, prevState) {
              if (view.state.doc.eq(prevState.doc)) return
              if (debounceTimer) clearTimeout(debounceTimer)
              debounceTimer = setTimeout(checkCompletion, 1500)
            },
            destroy() {
              if (debounceTimer) clearTimeout(debounceTimer)
            },
          }
        },
      }),
    ]
  },
})

const AiCompletePluginKey = new PluginKey('aiComplete')

function buildDecoration(state: any, text: string): DecorationSet {
  const { from } = state.selection
  const deco = Decoration.widget(from, () => {
    const span = document.createElement('span')
    span.className = 'ai-complete-ghost'
    span.textContent = text
    return span
  })
  return DecorationSet.create(state.doc, [deco])
}
