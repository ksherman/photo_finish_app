# Script for populating the database. You can run it as:
#
#     mix run priv/repo/seeds.exs
#
# Inside the script, you can read and write to any of your
# repositories directly:
#
#     PhotoFinish.Repo.insert!(%PhotoFinish.SomeSchema{})
#
# We recommend using the bang functions (`insert!`, `update!`
# and so on) as they will fail if something goes wrong.

alias PhotoFinish.Orders.ProductTemplate

# Default product templates (idempotent — skips if already seeded)
if Ash.read!(ProductTemplate) == [] do
  templates = [
    %{product_type: :usb, product_name: "All Photos USB Drive", default_price_cents: 10_000, display_order: 0},
    %{product_type: :print, product_name: "5x7 Print", product_size: "5x7", default_price_cents: 1_800, display_order: 1},
    %{product_type: :print, product_name: "8x10 Print", product_size: "8x10", default_price_cents: 3_000, display_order: 2},
    %{product_type: :print, product_name: "11x14 Print", product_size: "11x14", default_price_cents: 4_000, display_order: 3},
    %{product_type: :print, product_name: "16x20 Print", product_size: "16x20", default_price_cents: 6_500, display_order: 4}
  ]

  for attrs <- templates do
    Ash.create!(ProductTemplate, attrs)
  end

  IO.puts("Seeded #{length(templates)} default product templates")
end
