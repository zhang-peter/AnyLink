from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
ASSETS.mkdir(parents=True, exist_ok=True)
SIZE = 1024
SS = 2
N = SIZE * SS


def lerp_color(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))


canvas = Image.new("RGBA", (N, N), (0, 0, 0, 0))
gradient = Image.new("RGBA", (N, N))
pixels = gradient.load()
top = (7, 16, 31)
bottom = (9, 43, 84)
for y in range(N):
    t = y / (N - 1)
    color = lerp_color(top, bottom, t)
    for x in range(N):
        # A soft diagonal lift towards the upper-right.
        diag = max(0.0, min(1.0, (x / N) * 0.35 + (1.0 - t) * 0.2))
        pixels[x, y] = (
            min(255, round(color[0] + 8 * diag)),
            min(255, round(color[1] + 15 * diag)),
            min(255, round(color[2] + 28 * diag)),
            255,
        )

mask = Image.new("L", (N, N), 0)
ImageDraw.Draw(mask).rounded_rectangle((48, 48, N - 48, N - 48), radius=214 * SS, fill=255)
canvas.alpha_composite(Image.composite(gradient, Image.new("RGBA", (N, N)), mask))

# Inner border.
border = Image.new("RGBA", (N, N), (0, 0, 0, 0))
ImageDraw.Draw(border).rounded_rectangle(
    (80, 80, N - 80, N - 80),
    radius=185 * SS,
    outline=(49, 130, 206, 90),
    width=5 * SS,
)
canvas.alpha_composite(border)

# Ambient glow behind the links.
glow = Image.new("RGBA", (N, N), (0, 0, 0, 0))
d = ImageDraw.Draw(glow)
d.ellipse((330 * SS, 320 * SS, 1720 * SS, 1700 * SS), fill=(26, 131, 255, 70))
d.ellipse((650 * SS, 550 * SS, 1450 * SS, 1450 * SS), fill=(68, 224, 255, 40))
glow = glow.filter(ImageFilter.GaussianBlur(110 * SS))
canvas.alpha_composite(glow)


def link_layer(size, center, angle, color, stroke):
    layer = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    half_w = size[0] // 2
    half_h = size[1] // 2
    left = center[0] - half_w
    top = center[1] - half_h
    draw.rounded_rectangle(
        (left, top, left + size[0], top + size[1]),
        radius=half_h,
        outline=color,
        width=stroke,
    )
    return layer.rotate(angle, resample=Image.Resampling.BICUBIC, center=center)


center = (N // 2, N // 2)
# Two broad chain links, interlocked at an angle.
link_a = link_layer((1040, 520), center, 27, (74, 196, 255, 255), 120)
link_b = link_layer((1040, 520), center, -27, (129, 140, 248, 255), 120)
canvas.alpha_composite(link_a)
canvas.alpha_composite(link_b)

# A small, bright connector in the middle of the chain.
connector = Image.new("RGBA", (N, N), (0, 0, 0, 0))
d = ImageDraw.Draw(connector)
r = 74 * SS
d.ellipse((center[0] - r, center[1] - r, center[0] + r, center[1] + r), fill=(236, 253, 255, 255))
d.ellipse(
    (center[0] - r + 25 * SS, center[1] - r + 25 * SS, center[0] + r - 25 * SS, center[1] + r - 25 * SS),
    fill=(56, 189, 248, 255),
)
canvas.alpha_composite(connector)

# A small download arrow badge at the bottom.
badge_y = 1600
badge_r = 170
badge = Image.new("RGBA", (N, N), (0, 0, 0, 0))
d = ImageDraw.Draw(badge)
d.ellipse(
    (center[0] - badge_r, badge_y - badge_r, center[0] + badge_r, badge_y + badge_r),
    fill=(8, 23, 43, 245),
    outline=(139, 233, 255, 255),
    width=10 * SS,
)
arrow = (231, 253, 255, 255)
stem_w = 26 * SS
d.rounded_rectangle(
    (center[0] - stem_w // 2, badge_y - 53 * SS, center[0] + stem_w // 2, badge_y + 25 * SS),
    radius=12 * SS,
    fill=arrow,
)
d.line(
    [(center[0] - 53 * SS, badge_y - 2 * SS), (center[0], badge_y + 50 * SS), (center[0] + 53 * SS, badge_y - 2 * SS)],
    fill=arrow,
    width=stem_w,
    joint="curve",
)
d.line(
    [(center[0] - 53 * SS, badge_y + 22 * SS), (center[0], badge_y + 72 * SS), (center[0] + 53 * SS, badge_y + 22 * SS)],
    fill=arrow,
    width=stem_w,
    joint="curve",
)
canvas.alpha_composite(badge)

logo = canvas.resize((SIZE, SIZE), Image.Resampling.LANCZOS)
logo.save(ASSETS / "logo.png", optimize=True)

icon_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
logo.save(ASSETS / "logo.ico", sizes=icon_sizes)
print(f"wrote {ASSETS / 'logo.png'} and {ASSETS / 'logo.ico'}")
