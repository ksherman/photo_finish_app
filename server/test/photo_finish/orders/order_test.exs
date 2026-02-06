defmodule PhotoFinish.Orders.OrderTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.Event
  alias PhotoFinish.Orders.Order

  setup do
    {:ok, event} =
      Ash.create(Event, %{
        name: "Test Event",
        slug: "test-event",
        storage_root: "/path/to/photos"
      })

    %{event: event}
  end

  describe "create" do
    test "generates ID with ord_ prefix", %{event: event} do
      {:ok, order} =
        Ash.create(Order, %{
          event_id: event.id,
          order_number: "ORD-001",
          customer_name: "Jane Doe",
          subtotal_cents: 5000,
          tax_rate_basis_points: 850,
          tax_cents: 425,
          total_cents: 5425
        })

      assert String.starts_with?(order.id, "ord_")
      suffix = String.replace_prefix(order.id, "ord_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end

    test "creates order with all required fields", %{event: event} do
      {:ok, order} =
        Ash.create(Order, %{
          event_id: event.id,
          order_number: "ORD-002",
          customer_name: "John Smith",
          subtotal_cents: 10000,
          tax_rate_basis_points: 850,
          tax_cents: 850,
          total_cents: 10850
        })

      assert order.order_number == "ORD-002"
      assert order.customer_name == "John Smith"
      assert order.subtotal_cents == 10000
      assert order.tax_rate_basis_points == 850
      assert order.tax_cents == 850
      assert order.total_cents == 10850
      assert order.event_id == event.id
    end

    test "creates order with optional fields", %{event: event} do
      {:ok, order} =
        Ash.create(Order, %{
          event_id: event.id,
          order_number: "ORD-003",
          customer_name: "Jane Doe",
          customer_email: "jane@example.com",
          customer_phone: "555-1234",
          subtotal_cents: 5000,
          tax_rate_basis_points: 850,
          tax_cents: 425,
          total_cents: 5425,
          payment_reference: "REF-123",
          notes: "Gift wrap requested"
        })

      assert order.customer_email == "jane@example.com"
      assert order.customer_phone == "555-1234"
      assert order.payment_reference == "REF-123"
      assert order.notes == "Gift wrap requested"
    end

    test "defaults payment_status to pending", %{event: event} do
      {:ok, order} =
        Ash.create(Order, %{
          event_id: event.id,
          order_number: "ORD-004",
          customer_name: "Jane Doe",
          subtotal_cents: 5000,
          tax_rate_basis_points: 850,
          tax_cents: 425,
          total_cents: 5425
        })

      assert order.payment_status == :pending
    end

    test "requires order_number", %{event: event} do
      result =
        Ash.create(Order, %{
          event_id: event.id,
          customer_name: "Jane Doe",
          subtotal_cents: 5000,
          tax_rate_basis_points: 850,
          tax_cents: 425,
          total_cents: 5425
        })

      assert {:error, _} = result
    end

    test "requires customer_name", %{event: event} do
      result =
        Ash.create(Order, %{
          event_id: event.id,
          order_number: "ORD-005",
          subtotal_cents: 5000,
          tax_rate_basis_points: 850,
          tax_cents: 425,
          total_cents: 5425
        })

      assert {:error, _} = result
    end

    test "enforces unique order_number", %{event: event} do
      {:ok, _} =
        Ash.create(Order, %{
          event_id: event.id,
          order_number: "ORD-UNIQUE",
          customer_name: "Jane Doe",
          subtotal_cents: 5000,
          tax_rate_basis_points: 850,
          tax_cents: 425,
          total_cents: 5425
        })

      result =
        Ash.create(Order, %{
          event_id: event.id,
          order_number: "ORD-UNIQUE",
          customer_name: "John Smith",
          subtotal_cents: 3000,
          tax_rate_basis_points: 850,
          tax_cents: 255,
          total_cents: 3255
        })

      assert {:error, _} = result
    end
  end

  describe "update" do
    test "updates order fields", %{event: event} do
      {:ok, order} =
        Ash.create(Order, %{
          event_id: event.id,
          order_number: "ORD-006",
          customer_name: "Jane Doe",
          subtotal_cents: 5000,
          tax_rate_basis_points: 850,
          tax_cents: 425,
          total_cents: 5425
        })

      {:ok, updated} =
        Ash.update(order, %{
          payment_status: :paid,
          payment_reference: "CARD-456",
          notes: "Paid in full"
        })

      assert updated.payment_status == :paid
      assert updated.payment_reference == "CARD-456"
      assert updated.notes == "Paid in full"
    end
  end
end
