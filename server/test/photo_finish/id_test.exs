defmodule PhotoFinish.IdTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Id

  describe "generate/1" do
    test "generates ID with correct format: prefix + 3 letters + 4 numbers" do
      id = Id.generate("test_")

      assert String.starts_with?(id, "test_")
      # After the prefix, should be 3 lowercase letters followed by 4 numbers
      suffix = String.replace_prefix(id, "test_", "")
      assert String.length(suffix) == 7
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end

    test "generates unique IDs" do
      ids = for _ <- 1..100, do: Id.generate("test_")
      unique_ids = Enum.uniq(ids)

      assert length(ids) == length(unique_ids), "Generated IDs should be unique"
    end
  end

  describe "event_id/0" do
    test "generates ID with evt_ prefix" do
      id = Id.event_id()

      assert String.starts_with?(id, "evt_")
      suffix = String.replace_prefix(id, "evt_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end
  end

  describe "photo_id/0" do
    test "generates ID with pho_ prefix" do
      id = Id.photo_id()

      assert String.starts_with?(id, "pho_")
      suffix = String.replace_prefix(id, "pho_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end
  end

  describe "competitor_id/0" do
    test "generates ID with cmp_ prefix" do
      id = Id.competitor_id()

      assert String.starts_with?(id, "cmp_")
      suffix = String.replace_prefix(id, "cmp_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end
  end

  describe "order_id/0" do
    test "generates ID with ord_ prefix" do
      id = Id.order_id()

      assert String.starts_with?(id, "ord_")
      suffix = String.replace_prefix(id, "ord_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end
  end

  describe "order_item_id/0" do
    test "generates ID with itm_ prefix" do
      id = Id.order_item_id()

      assert String.starts_with?(id, "itm_")
      suffix = String.replace_prefix(id, "itm_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end
  end

  describe "product_id/0" do
    test "generates ID with prd_ prefix" do
      id = Id.product_id()

      assert String.starts_with?(id, "prd_")
      suffix = String.replace_prefix(id, "prd_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end
  end
end
