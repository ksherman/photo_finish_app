defmodule PhotoFinish.Orders.OrderNumberTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.Event
  alias PhotoFinish.Orders.OrderNumber

  describe "generate/1" do
    test "formats order number as order_code-NNNN" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Spring Invitational",
          slug: "spring-inv",
          storage_root: "/photos/spring",
          order_code: "STV"
        })

      {:ok, order_number} = OrderNumber.generate(event.id)
      assert order_number == "STV-0001"
    end

    test "sequential calls produce incrementing numbers" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Fall Classic",
          slug: "fall-classic",
          storage_root: "/photos/fall",
          order_code: "FCL"
        })

      {:ok, first} = OrderNumber.generate(event.id)
      {:ok, second} = OrderNumber.generate(event.id)
      {:ok, third} = OrderNumber.generate(event.id)

      assert first == "FCL-0001"
      assert second == "FCL-0002"
      assert third == "FCL-0003"
    end

    test "pads numbers correctly" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Padding Test",
          slug: "pad-test",
          storage_root: "/photos/pad",
          order_code: "PAD"
        })

      # Generate 42 numbers to test padding at various points
      results =
        for _i <- 1..42 do
          {:ok, num} = OrderNumber.generate(event.id)
          num
        end

      assert Enum.at(results, 0) == "PAD-0001"
      assert Enum.at(results, 8) == "PAD-0009"
      assert Enum.at(results, 9) == "PAD-0010"
      assert Enum.at(results, 41) == "PAD-0042"
    end

    test "handles numbers beyond 4 digits" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Large Event",
          slug: "large-event",
          storage_root: "/photos/large",
          order_code: "LRG"
        })

      # Manually set the counter high to test beyond 4 digits
      Ecto.Adapters.SQL.query!(
        PhotoFinish.Repo,
        "UPDATE events SET next_order_number = 9998 WHERE id = $1",
        [event.id]
      )

      {:ok, num1} = OrderNumber.generate(event.id)
      {:ok, num2} = OrderNumber.generate(event.id)

      assert num1 == "LRG-9999"
      assert num2 == "LRG-10000"
    end

    test "returns error for non-existent event" do
      assert {:error, :event_not_found} = OrderNumber.generate("evt_nonexistent")
    end

    test "works with nil order_code" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "No Code Event",
          slug: "no-code",
          storage_root: "/photos/nocode"
        })

      {:ok, order_number} = OrderNumber.generate(event.id)
      assert order_number == "-0001"
    end
  end
end
