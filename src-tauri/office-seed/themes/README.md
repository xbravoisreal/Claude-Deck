# Theme cua Office

Moi theme la mot thu muc trong `themes/`. Chon theme trong Settings (⚙).

## Tao theme moi

1. Copy `themes/default` thanh `themes/<ten-moi>`.
2. Sua `theme.json`:
   - `colors`: mau giao dien (`bg`, `room`, `border`, `text`, `muted`, `accent`, `review`, `waiting`, `running`, `idle`, `warn`, `bubble-text`).
   - `font`: font chu.
   - `sprite.frame`: kich thuoc mot khung anh (px). `sprite.scale`: phong to bao nhieu lan.
   - `sprite.characterY`, `sprite.deskY`: dich nhan vat / ban len xuong (px cua khung) cho dau nhan vat lo tren ban.
   - `slots`: ten slot -> file anh (SVG, PNG, GIF, WEBP) nam trong thu muc theme.
3. Thay anh. Chi can ghi nhung slot muon doi; slot thieu se lay tu theme mac dinh.

## Slot

| Slot | Noi dung |
|---|---|
| `wall` | o tuong (hang tren cung cua phong) |
| `floor.<id>` | o san, phong chon bang `room.json` -> `floor` |
| `desk` | ban lam viec, ve de len nua duoi nhan vat |
| `character.<id>.typing` / `.waiting` / `.idle` | nhan vat: dang lam / cho sep / ranh. Anh nhieu khung xep ngang, moi khung `frame` px, tu chay animation |
| `decor.<id>` | do trang tri, dat bang `room.json` -> `decor` |

## Bo tri phong: `departments/<phong>/room.json`

```json
{
  "cols": 8, "rows": 4, "floor": "floor.wood",
  "decor": [{ "slot": "decor.plant", "x": 0, "y": 1 }],
  "seats": { "dev": { "x": 2, "y": 1, "character": "character.c2" } }
}
```

- `x`, `y`: o tinh tu goc trai tren (hang 0 la tuong).
- Agent khong co trong `seats` duoc xep tu dong vao o trong. Bo `character` thi chon theo ten Agent.
- Sua xong bam tab Office lai (hoac doi theme) de tai lai.

Theme `default` do Claude ve bang `scripts/gen-theme.mjs` (CC0, dung tu do).
