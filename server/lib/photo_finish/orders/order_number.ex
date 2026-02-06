defmodule PhotoFinish.Orders.OrderNumber do
  @moduledoc """
  Generates sequential order numbers for an event.

  Atomically increments the event's `next_order_number` counter and
  formats the result as `"{order_code}-{NNNN}"`.
  """

  alias PhotoFinish.Repo

  @doc """
  Generates the next order number for the given event.

  Atomically increments `events.next_order_number` and returns a
  formatted string like `"STV-0001"`.

  ## Parameters

    * `event_id` - The ID of the event to generate a number for.

  ## Returns

    * `{:ok, order_number}` on success
    * `{:error, reason}` on failure

  ## Examples

      iex> generate("evt_abc1234")
      {:ok, "STV-0001"}
  """
  @spec generate(String.t()) :: {:ok, String.t()} | {:error, term()}
  def generate(event_id) when is_binary(event_id) do
    query = """
    UPDATE events
    SET next_order_number = next_order_number + 1,
        updated_at = NOW() AT TIME ZONE 'utc'
    WHERE id = $1
    RETURNING next_order_number, order_code
    """

    case Ecto.Adapters.SQL.query(Repo, query, [event_id]) do
      {:ok, %{num_rows: 1, rows: [[seq, order_code]]}} ->
        padded = seq |> to_string() |> String.pad_leading(4, "0")
        {:ok, "#{order_code}-#{padded}"}

      {:ok, %{num_rows: 0}} ->
        {:error, :event_not_found}

      {:error, reason} ->
        {:error, reason}
    end
  end
end
