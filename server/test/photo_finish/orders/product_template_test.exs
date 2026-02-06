defmodule PhotoFinish.Orders.ProductTemplateTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Orders.ProductTemplate

  describe "create" do
    test "generates ID with ptm_ prefix" do
      {:ok, template} =
        Ash.create(ProductTemplate, %{
          product_type: :print,
          product_name: "8x10 Print",
          default_price_cents: 2500
        })

      assert String.starts_with?(template.id, "ptm_")
      suffix = String.replace_prefix(template.id, "ptm_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end

    test "creates template with all fields" do
      {:ok, template} =
        Ash.create(ProductTemplate, %{
          product_type: :print,
          product_name: "5x7 Print",
          product_size: "5x7",
          default_price_cents: 1500,
          is_active: true,
          display_order: 2
        })

      assert template.product_type == :print
      assert template.product_name == "5x7 Print"
      assert template.product_size == "5x7"
      assert template.default_price_cents == 1500
      assert template.is_active == true
      assert template.display_order == 2
    end

    test "defaults is_active to true and display_order to 0" do
      {:ok, template} =
        Ash.create(ProductTemplate, %{
          product_type: :usb,
          product_name: "USB Drive",
          default_price_cents: 5000
        })

      assert template.is_active == true
      assert template.display_order == 0
    end

    test "allows null product_size" do
      {:ok, template} =
        Ash.create(ProductTemplate, %{
          product_type: :usb,
          product_name: "USB Drive",
          default_price_cents: 5000
        })

      assert template.product_size == nil
    end

    test "requires product_type" do
      result =
        Ash.create(ProductTemplate, %{
          product_name: "Missing Type",
          default_price_cents: 1000
        })

      assert {:error, _} = result
    end

    test "requires product_name" do
      result =
        Ash.create(ProductTemplate, %{
          product_type: :print,
          default_price_cents: 1000
        })

      assert {:error, _} = result
    end

    test "requires default_price_cents" do
      result =
        Ash.create(ProductTemplate, %{
          product_type: :print,
          product_name: "Print"
        })

      assert {:error, _} = result
    end

    test "validates product_type enum values" do
      for type <- [:usb, :print, :collage, :custom_photo, :accessory] do
        {:ok, template} =
          Ash.create(ProductTemplate, %{
            product_type: type,
            product_name: "#{type} product",
            default_price_cents: 1000
          })

        assert template.product_type == type
      end
    end
  end

  describe "read" do
    test "reads all templates" do
      {:ok, _} =
        Ash.create(ProductTemplate, %{
          product_type: :print,
          product_name: "8x10 Print",
          default_price_cents: 2500
        })

      templates = Ash.read!(ProductTemplate)
      assert length(templates) >= 1
    end
  end

  describe "update" do
    test "updates template fields" do
      {:ok, template} =
        Ash.create(ProductTemplate, %{
          product_type: :print,
          product_name: "8x10 Print",
          default_price_cents: 2500
        })

      {:ok, updated} =
        Ash.update(template, %{
          product_name: "8x10 Glossy Print",
          default_price_cents: 3000,
          is_active: false
        })

      assert updated.product_name == "8x10 Glossy Print"
      assert updated.default_price_cents == 3000
      assert updated.is_active == false
    end
  end

  describe "destroy" do
    test "deletes a template" do
      {:ok, template} =
        Ash.create(ProductTemplate, %{
          product_type: :accessory,
          product_name: "Keychain",
          default_price_cents: 800
        })

      assert :ok = Ash.destroy(template)
      assert Ash.read!(ProductTemplate) |> Enum.find(&(&1.id == template.id)) == nil
    end
  end
end
