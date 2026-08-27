---
title: Math and Diagrams
tags:
  - reference
  - kbview/markdown
  - math
date: 2026-08-19
aliases:
  - KaTeX
  - Mermaid
---

# Math and Diagrams

## Inline math

The area of a square with side $x$ is $x^2$, and Euler's identity is
$e^{i\pi} + 1 = 0$. Inline math sits inside a sentence, so $\alpha + \beta$ must
not break the surrounding paragraph.

## Display math

$$
\int_{-\infty}^{\infty} e^{-x^2} \, dx = \sqrt{\pi}
$$

A multi-line display block:

$$
\begin{aligned}
f(x) &= (x + 1)^2 \\
     &= x^2 + 2x + 1
\end{aligned}
$$

## Not math

A price list must not become math: it costs $5 today and $7 tomorrow.
A single dollar inside code is safe too: `$HOME` and `$PATH`.

## Mermaid

```mermaid
sequenceDiagram
    participant Browser
    participant Server
    participant Disk
    Browser->>Server: GET /root/obsidian-vault/index.md
    Server->>Disk: read index.md
    Disk-->>Server: bytes
    Server->>Server: parse frontmatter, resolve wikilinks
    Server-->>Browser: rendered HTML
```

A second mermaid diagram, of the vault's own shape:

```mermaid
graph LR
    index[index.md] --> glossary[Glossary]
    index --> overview[projects/kbview/overview]
    overview --> pipeline[rendering-pipeline]
    pipeline --> adr[adr-001-markdown-engine]
    index -.unresolved.-> missing[Does Not Exist]
```

## An embedded SVG attachment

![[diagram.svg]]
