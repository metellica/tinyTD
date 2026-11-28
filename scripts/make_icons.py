# 生成 tinyTD 应用图标（圆角渐变底 + 白色对勾）
# 用法: python scripts/make_icons.py
import os

from PIL import Image, ImageDraw

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src-tauri", "icons")
os.makedirs(OUT, exist_ok=True)

S = 512
TOP, BOT = (99, 102, 241), (139, 92, 246)  # 靛蓝 -> 紫

# 垂直渐变
grad = Image.new("RGBA", (S, S))
d = ImageDraw.Draw(grad)
for y in range(S):
    t = y / (S - 1)
    c = tuple(int(TOP[i] + (BOT[i] - TOP[i]) * t) for i in range(3)) + (255,)
    d.line([(0, y), (S, y)], fill=c)

# 圆角蒙版
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle([6, 6, S - 6, S - 6], radius=116, fill=255)

icon = Image.new("RGBA", (S, S), (0, 0, 0, 0))
icon.paste(grad, (0, 0), mask)

# 白色对勾（圆头线帽）
d = ImageDraw.Draw(icon)
pts = [(150, 268), (233, 350), (366, 208)]
d.line(pts, fill=(255, 255, 255, 255), width=44, joint="curve")
r = 22
for (x, y) in (pts[0], pts[-1]):
    d.ellipse([x - r, y - r, x + r, y + r], fill=(255, 255, 255, 255))

def padded(n, ratio):
    """把满幅图标缩放到 n*ratio 并居中放在 n x n 透明画布上"""
    body = round(n * ratio * S / (S - 12))
    out = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    off = (n - body) // 2
    out.paste(icon.resize((body, body), Image.LANCZOS), (off, off))
    return out


# macOS 图标网格：1024 画布中主体 824x824（四周留白约 100），否则在启动台/Dock 中会显得偏大
mac = padded(S, 824 / 1024)

mac.save(os.path.join(OUT, "icon.png"))
mac.resize((256, 256), Image.LANCZOS).save(os.path.join(OUT, "128x128@2x.png"))
mac.resize((128, 128), Image.LANCZOS).save(os.path.join(OUT, "128x128.png"))
mac.resize((32, 32), Image.LANCZOS).save(os.path.join(OUT, "32x32.png"))

# Windows ICO：主体约占 7/8，与任务栏其它图标视觉大小一致；每个尺寸单独生成保证小图清晰
ico_sizes = [16, 24, 32, 48, 64, 128, 256]
ico_imgs = [padded(n, 0.875) for n in ico_sizes]
ico_imgs[-1].save(
    os.path.join(OUT, "icon.ico"),
    sizes=[(n, n) for n in ico_sizes],
    append_images=ico_imgs[:-1],
)

# 托盘图标（Windows/Linux）：彩色、满幅
icon.resize((32, 32), Image.LANCZOS).save(os.path.join(OUT, "tray.png"))

# macOS 菜单栏模板图标：黑色圆角方块 + 镂空对勾，系统按 alpha 自动适配深/浅色菜单栏
alpha = Image.new("L", (S, S), 0)
ImageDraw.Draw(alpha).rounded_rectangle([6, 6, S - 6, S - 6], radius=116, fill=255)
ad = ImageDraw.Draw(alpha)
ad.line(pts, fill=0, width=56, joint="curve")
r = 28
for (x, y) in (pts[0], pts[-1]):
    ad.ellipse([x - r, y - r, x + r, y + r], fill=0)
tpl = Image.new("RGBA", (S, S), (0, 0, 0, 0))
tpl.putalpha(alpha)
tpl.resize((44, 44), Image.LANCZOS).save(os.path.join(OUT, "tray-template.png"))
print("icons written to", os.path.abspath(OUT))
