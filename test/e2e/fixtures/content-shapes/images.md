# Images

A markdown image far wider than any viewport. It must scale down to the column, never
overflow it and never be clipped:

![A wide screenshot](assets/wide-screenshot.png)

The same image as an Obsidian embed, which takes a different code path:

![[assets/wide-screenshot.png]]

An image taller than a phone screen:

![A tall diagram](assets/tall-diagram.png)

A small image must NOT be stretched up to fill the column:

![A small square](assets/pixel.png)

An SVG, which has no intrinsic pixel size:

![A blue square](assets/shape.svg)

An image inside a link:

[![A wide screenshot](assets/wide-screenshot.png)](assets/wide-screenshot.png)
