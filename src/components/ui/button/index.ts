import type { VariantProps } from 'class-variance-authority'
import { cva } from 'class-variance-authority'

export { default as Button } from './Button.vue'

// chunky game-menu buttons: a 2px dark edge plus an inset bottom shade that flattens on press
export const buttonVariants = cva(
  'focus-visible:ring-ring/50 aria-invalid:border-destructive rounded-md border-2 font-extrabold focus-visible:ring-3 active:not-aria-[haspopup]:translate-y-px active:shadow-none [&_svg:not([class*=size-])]:size-4 group/button inline-flex shrink-0 items-center justify-center whitespace-nowrap transition-all outline-none select-none disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0',
  {
    variants: {
      variant: {
        default: 'bg-primary text-primary-foreground border-wood-dark shadow-[inset_0_-3px_0_var(--gold-shade)] hover:brightness-105',
        play: 'bg-success text-success-foreground border-[#23491a] shadow-[inset_0_-4px_0_var(--success-shade)] font-pixel hover:brightness-110',
        outline: 'bg-card text-foreground border-wood-dark shadow-[inset_0_-3px_0_#e2c08a] hover:bg-accent aria-expanded:bg-accent',
        secondary: 'bg-secondary text-secondary-foreground border-input hover:bg-accent',
        ghost: 'border-transparent hover:bg-accent/60 aria-expanded:bg-accent',
        destructive: 'bg-card text-destructive border-destructive shadow-[inset_0_-3px_0_#e8b4a6] hover:bg-warning-soft',
        link: 'border-transparent text-[#7a3e12] underline-offset-4 hover:underline',
      },
      size: {
        'default': 'h-9 gap-1.5 px-3.5 text-sm',
        'xs': 'h-6 gap-1 px-2 text-xs [&_svg:not([class*=size-])]:size-3',
        'sm': 'h-8 gap-1 px-2.5 text-[0.8rem] [&_svg:not([class*=size-])]:size-3.5',
        'lg': 'h-10 gap-2 px-4 text-[15px]',
        'xl': 'h-13 gap-2.5 px-5 text-2xl [&_svg:not([class*=size-])]:size-5',
        'icon': 'size-9',
        'icon-xs': 'size-6 [&_svg:not([class*=size-])]:size-3',
        'icon-sm': 'size-8',
        'icon-lg': 'size-10',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
)
export type ButtonVariants = VariantProps<typeof buttonVariants>
