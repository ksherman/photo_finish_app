defmodule PhotoFinish.Id do
  @moduledoc """
  Generates prefixed short IDs in the format: prefix_abc1234
  (3 lowercase letters + 4 numbers)
  """

  @letter_alphabet "abcdefghijklmnopqrstuvwxyz"
  @number_alphabet "0123456789"

  def generate(prefix) when is_binary(prefix) do
    letters = Nanoid.generate(3, @letter_alphabet)
    numbers = Nanoid.generate(4, @number_alphabet)
    "#{prefix}#{letters}#{numbers}"
  end

  # Convenience functions for each entity type
  def event_id, do: generate("evt_")
  def photo_id, do: generate("pho_")
  def competitor_id, do: generate("cmp_")
  def event_competitor_id, do: generate("evc_")
  def order_id, do: generate("ord_")
  def order_item_id, do: generate("itm_")
  def product_id, do: generate("prd_")
end
