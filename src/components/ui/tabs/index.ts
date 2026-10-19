import type { VariantProps } from 'class-variance-authority'
import { cva } from 'class-variance-authority'

export { default as Tabs } from './Tabs.vue'
export { default as TabsContent } from './TabsContent.vue'
export { default as TabsList } from './TabsList.vue'
export { default as TabsTrigger } from './TabsTrigger.vue'

export const tabsListVariants = cva(
  'group/tabs-list inline-flex w-fit flex-wrap items-center gap-1 text-muted-foreground group-data-vertical/tabs:flex-col',
  {
    variants: {
      variant: {
        default: 'border-b-2 border-dashed border-border-soft pb-1.5',
        line: 'gap-1 bg-transparent',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  },
)

export type TabsListVariants = VariantProps<typeof tabsListVariants>
