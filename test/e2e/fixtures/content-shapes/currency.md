# Currency, not mathematics

A permissive `$…$` scanner pairs the first amount with the second and swallows the prose
between them into a MathML blob — silently, because the page still renders.

The coffee costs $5 today and $7 tomorrow, and this sentence must survive intact.

Inline maths still works when it does not open with a digit: $x^2 + y^2 = z^2$.

Deliberately literal, because it opens with a digit: $5x$.
