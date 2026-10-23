# Rust/UI and Liquid Glass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Apple-style Liquid Glass optical system and Rust/UI high-density component primitives across VolumeControl's webview surfaces (Mixer, Settings, Help, Overlay) with zero runtime JS dependency additions and strict WCAG AA contrast compliance.

**Architecture:** Port Rust/UI component structures and Tailwind CSS v4 design tokens to VolumeControl's existing React 19 webview frontend. Enforce a 4-tier elevation hierarchy (Z0–Z3) where the outer Surface Shell executes a single-pass 20px backdrop blur with specular rim lighting, while nested content cards use translucent OKLab luminance tints without nested blur (Anti-Blur Multiplication constraint, ADR 0002).

**Tech Stack:** React 19, TypeScript 5.8, Tailwind CSS v4, Radix UI primitives, Vite 6, Vitest 4, Tauri v2 IPC bridge.

## Global Constraints
- Target bundle CSS size increase must not exceed 2.5 kB gzipped.
- Zero added runtime npm packages.
- Anti-Blur Multiplication rule (ADR 0002): No element inside `.glass-surface` may declare `backdrop-filter: blur()`.
- Single source of truth for configuration is `config.ini` in the platform app directory; never invent or write `config.json`.
- Reduced-motion invariant: `.reduced-motion *` forces duration to `0.001s !important`.
- Bilingual parity: All new appearance and material UI copy must be localized in English and Vietnamese in `frontend/src/lib/i18n.tsx`.
- All 43 existing Vitest test suites must continue to pass 100%.

---

### Task 1: Optical Liquid Glass CSS Tokens & Surface Shell Upgrade

**Files:**
- Modify: `frontend/src/styles.css:96-163`
- Test: `frontend/src/styles.test.ts`

**Interfaces:**
- Consumes: Tailwind v4 `@theme inline`, CSS variables `--background`, `--foreground`, `--card`, `--border`.
- Produces: Enhanced `.glass-surface`, `.surface-shell`, `--glass-blur`, `--glass-specular`, and squircle curvature utilities.

- [ ] **Step 1: Write failing test for enhanced glass surface tokens**

Add tests in `frontend/src/styles.test.ts` verifying that `.glass-surface` contains 20px blur, saturation boost, and specular rim box-shadow definitions:

```typescript
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

describe('Liquid Glass CSS Tokens', () => {
  it('defines 20px blur and specular rim in styles.css', () => {
    const css = fs.readFileSync(path.resolve(__dirname, 'styles.css'), 'utf-8');
    expect(css).toContain('blur(20px)');
    expect(css).toContain('saturate(180%)');
    expect(css).toContain('inset 0 1px 0 0');
  });

  it('preserves single-pass anti-blur constraint', () => {
    const css = fs.readFileSync(path.resolve(__dirname, 'styles.css'), 'utf-8');
    const matches = css.match(/backdrop-filter:\s*blur/g);
    // Only declared within .glass-surface @supports blocks
    expect(matches).not.toBeNull();
    expect(matches!.length).toBeLessThanOrEqual(2);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test -- frontend/src/styles.test.ts`
Expected: FAIL with `expected css to contain 'blur(20px)'`.

- [ ] **Step 3: Implement Liquid Glass CSS tokens in `frontend/src/styles.css`**

Update `.glass-surface` in `frontend/src/styles.css`:

```css
/* ── Liquid Glass surfaces (optical refraction, specular rim, progressive enhancement) ── */
@layer utilities {
  .surface-shell {
    height: 100dvh;
    min-height: 0;
    overflow: hidden;
    background-color: color-mix(in oklab, var(--background) 94%, transparent);
    color: var(--foreground);
    border-radius: 1.125rem; /* 18px continuous squircle */
  }

  .glass-surface {
    /* Fallback for disabled compositing or missing backdrop-filter */
    background-color: color-mix(in oklab, var(--background) 94%, transparent);
    border: 1px solid color-mix(in oklab, var(--border) 40%, transparent);
  }

  @supports ((-webkit-backdrop-filter: blur(20px)) or (backdrop-filter: blur(20px))) {
    .glass-surface {
      -webkit-backdrop-filter: blur(20px) saturate(180%);
      backdrop-filter: blur(20px) saturate(180%);
      background-color: color-mix(in oklab, var(--background) 82%, transparent);
      box-shadow:
        inset 0 1px 0 0 rgba(255, 255, 255, 0.22),
        0 16px 36px -4px rgba(0, 0, 0, 0.4);
      border: 1px solid color-mix(in oklab, var(--foreground) 10%, transparent);
    }
  }

  :root:not(.dark) .glass-surface {
    box-shadow:
      inset 0 1px 0 0 rgba(255, 255, 255, 0.70),
      0 16px 36px -4px rgba(20, 24, 40, 0.12);
    border: 1px solid color-mix(in oklab, var(--foreground) 8%, transparent);
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm run test -- frontend/src/styles.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/styles.css frontend/src/styles.test.ts
git commit -m "feat(ui): implement multi-stage Liquid Glass CSS tokens"
```

---

### Task 2: Port Rust/UI `CardSm` and Tactile Pill Primitives

**Files:**
- Modify: `frontend/src/components/ui/card.tsx`
- Modify: `frontend/src/components/ui/badge.tsx`
- Create: `frontend/src/components/ui/card.test.tsx`

**Interfaces:**
- Consumes: `clsx`, `tailwind-merge`.
- Produces: `CardSm`, `Card`, `Badge` with `pill` variant and tactile active compression.

- [ ] **Step 1: Write failing test for CardSm and pill badge**

Create `frontend/src/components/ui/card.test.tsx`:

```typescript
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { CardSm, CardContent } from './card';
import { Badge } from './badge';

describe('Rust/UI Primitives', () => {
  it('renders CardSm with high-density padding and no nested blur', () => {
    const { container } = render(
      <CardSm data-testid="dense-card">
        <CardContent>Content</CardContent>
      </CardSm>
    );
    const el = screen.getByTestId('dense-card');
    expect(el.className).toContain('p-3');
    expect(el.className).toContain('bg-card/60');
    expect(el.className).not.toContain('backdrop-blur');
  });

  it('renders Badge with pill variant', () => {
    render(<Badge variant="pill">Active</Badge>);
    const badge = screen.getByText('Active');
    expect(badge.className).toContain('rounded-full');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test -- frontend/src/components/ui/card.test.tsx`
Expected: FAIL with `CardSm is not exported from './card'`.

- [ ] **Step 3: Implement `CardSm` and `pill` Badge**

Update `frontend/src/components/ui/card.tsx` to export `CardSm`:

```tsx
import * as React from "react";
import { cn } from "../../lib/utils";

const Card = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn(
      "rounded-2xl border border-border/50 bg-card text-card-foreground shadow-sm",
      className
    )}
    {...props}
  />
));
Card.displayName = "Card";

const CardSm = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn(
      "rounded-xl border border-border/40 bg-card/60 p-3 text-card-foreground shadow-sm transition-colors",
      className
    )}
    {...props}
  />
));
CardSm.displayName = "CardSm";

// Export subcomponents
export { Card, CardSm };
```

Update `frontend/src/components/ui/badge.tsx` to include `pill` variant:

```tsx
const badgeVariants = cva(
  "inline-flex items-center border font-semibold transition-all focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
  {
    variants: {
      variant: {
        default: "border-transparent bg-primary text-primary-foreground shadow hover:bg-primary/80",
        secondary: "border-transparent bg-secondary text-secondary-foreground hover:bg-secondary/80",
        pill: "rounded-full px-2.5 py-0.5 text-xs border-border/50 bg-secondary/70 text-secondary-foreground active:scale-[0.97] transition-transform",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm run test -- frontend/src/components/ui/card.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/ui/card.tsx frontend/src/components/ui/badge.tsx frontend/src/components/ui/card.test.tsx
git commit -m "feat(ui): port Rust/UI CardSm and pill badge primitives"
```

---

### Task 3: Volume Mixer HUD Liquid Glass & High-Density Migration

**Files:**
- Modify: `frontend/src/mixer/MixerSurface.tsx`
- Modify: `frontend/src/mixer/SystemOutputRow.tsx`
- Modify: `frontend/src/mixer/SessionRow.tsx`
- Test: `frontend/src/mixer/MixerSurface.test.tsx`
- Test: `frontend/src/mixer/SystemOutputRow.test.tsx`

**Interfaces:**
- Consumes: `CardSm`, `.glass-surface`, `SignalRail`, `useSessionStore`.
- Produces: Modernized 400×224px Volume Mixer HUD with visible dual session rows and tactile controls.

- [ ] **Step 1: Write failing test for Mixer high-density session rows**

Update `frontend/src/mixer/MixerSurface.test.tsx` to assert that master output and session rows render using `CardSm` structure and maintain high visual density:

```typescript
it('renders master row and session rows with CardSm density styling', () => {
  render(<MixerSurface />);
  const masterRow = screen.getByTestId('system-output-row');
  expect(masterRow.className).toContain('rounded-xl');
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test -- frontend/src/mixer/MixerSurface.test.tsx`
Expected: FAIL if `system-output-row` does not use the updated rounded-xl class.

- [ ] **Step 3: Update `MixerSurface.tsx`, `SystemOutputRow.tsx`, and `SessionRow.tsx`**

Integrate `CardSm` into `SystemOutputRow.tsx`:
```tsx
import { CardSm } from "../components/ui/card";
// Replace plain div container with CardSm
<CardSm data-testid="system-output-row" className="mb-2 border-border/40 bg-card/60 p-2.5">
  {/* Content */}
</CardSm>
```

Integrate `CardSm` into `SessionRow.tsx`:
```tsx
<CardSm data-testid={`session-row-${session.id}`} className="mb-1.5 border-border/30 bg-card/50 p-2 hover:border-accent/40 transition-colors">
  {/* Controls */}
</CardSm>
```

- [ ] **Step 4: Run mixer tests to verify they pass**

Run: `npm run test -- frontend/src/mixer/MixerSurface.test.tsx frontend/src/mixer/SystemOutputRow.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/mixer/
git commit -m "feat(mixer): modernize HUD with Liquid Glass and CardSm rows"
```

---

### Task 4: Settings Appearance Preview & Bilingual Localizations

**Files:**
- Modify: `frontend/src/settings/AppearanceSection.tsx`
- Modify: `frontend/src/settings/AppearancePreview.tsx`
- Modify: `frontend/src/lib/i18n.tsx`
- Test: `frontend/src/settings/AppearanceSection.test.tsx`
- Test: `frontend/src/lib/i18n.test.ts`

**Interfaces:**
- Consumes: `AppState.theme`, `AppState.material`, `AppState.motion`, `AppState.accent`, `useTranslation`.
- Produces: Real-time Liquid Glass interactive preview card and complete bilingual English/Vietnamese dictionaries.

- [ ] **Step 1: Write failing test for i18n material keys and preview card**

Update `frontend/src/lib/i18n.test.ts`:

```typescript
it('provides accurate Vietnamese translations for Liquid Glass tokens', () => {
  const { t } = useTestTranslation('vi');
  expect(t('appearance.material.translucent')).toBe('Kính chất lỏng (Trong mờ)');
  expect(t('appearance.specularRim')).toBe('Viền phản xạ khúc xạ');
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test -- frontend/src/lib/i18n.test.ts`
Expected: FAIL with missing translation keys.

- [ ] **Step 3: Implement translations and AppearancePreview simulation**

Update `frontend/src/lib/i18n.tsx` adding Vietnamese and English dictionary entries for material, motion, and accent controls:
```typescript
en: {
  'appearance.material.auto': 'Auto (System Native)',
  'appearance.material.translucent': 'Liquid Glass (Translucent)',
  'appearance.material.opaque': 'Solid (Opaque)',
  'appearance.specularRim': 'Specular Rim Refraction',
  'appearance.previewTitle': 'Liquid Glass Live Preview',
},
vi: {
  'appearance.material.auto': 'Tự động (Hệ điều hành)',
  'appearance.material.translucent': 'Kính chất lỏng (Trong mờ)',
  'appearance.material.opaque': 'Đục (Nguyên khối)',
  'appearance.specularRim': 'Viền phản xạ khúc xạ',
  'appearance.previewTitle': 'Xem trước Kính chất lỏng',
}
```

Update `AppearancePreview.tsx` to render the optical glass card with specular rim and live accent tint.

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm run test -- frontend/src/lib/i18n.test.ts frontend/src/settings/AppearanceSection.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/lib/i18n.tsx frontend/src/settings/AppearanceSection.tsx frontend/src/settings/AppearancePreview.tsx
git commit -m "feat(settings): add Liquid Glass preview and bilingual i18n"
```

---

### Task 5: Overlay HUD, Help Modernization & Full Production Verification

**Files:**
- Modify: `frontend/src/overlay/OverlaySurface.tsx`
- Modify: `frontend/src/help/HelpSurface.tsx`
- Test: All 43 test suites across `frontend/`

**Interfaces:**
- Consumes: All updated tokens, primitives, and bilingual strings.
- Produces: Clean, verified production build matching all acceptance criteria.

- [ ] **Step 1: Write failing test for Overlay capsule and Help keycaps**

Update `frontend/src/overlay/OverlaySurface.test.tsx` to verify `glass-surface` class and squircle geometry.

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test -- frontend/src/overlay/OverlaySurface.test.tsx`

- [ ] **Step 3: Update `OverlaySurface.tsx` and `HelpSurface.tsx`**

Apply capsule glass styling to Overlay container and tactile `Kbd` styling to Help shortcut cheat-sheet.

- [ ] **Step 4: Run full test suite and production build**

Run:
```bash
npm run test
npm run build
```
Expected: All tests pass (43+ files); production build finishes with CSS gzipped payload ≤ 9.5 kB.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/overlay/ frontend/src/help/
git commit -m "feat(ui): complete Liquid Glass rollout on Overlay and Help"
```

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-10-23-rust-ui-liquid-glass-implementation.md`. Two execution options:

1. **Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints
