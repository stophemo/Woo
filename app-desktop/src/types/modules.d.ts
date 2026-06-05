declare module 'markmap-view' {
  const Markmap: any
  export { Markmap }
}

declare module 'markmap-common' {
  export type INode = any
  export type IPureNode = any
  export interface MarkmapOptions {}
}

declare module 'turndown' {
  interface TurndownOptions {
    headingStyle?: 'setext' | 'atx'
    hr?: string
    bulletListMarker?: '-' | '+' | '*'
    codeBlockStyle?: 'indented' | 'fenced'
    emDelimiter?: '_' | '*'
    strongDelimiter?: '**' | '__'
    linkStyle?: 'inlined' | 'referenced'
    linkReferenceStyle?: 'full' | 'collapsed' | 'shortcut'
  }

  interface TurndownService {
    turndown(html: string): string
    use(plugin: any): TurndownService
    addRule(key: string, rule: any): TurndownService
    keep(filter: string | string[]): TurndownService
    remove(filter: string | string[]): TurndownService
  }

  interface TurndownStatic {
    new (options?: TurndownOptions): TurndownService
  }

  const Turndown: TurndownStatic
  export default Turndown
}

declare module 'turndown-plugin-gfm' {
  const tables: any
  const strikethrough: any
  export { tables, strikethrough }
}
