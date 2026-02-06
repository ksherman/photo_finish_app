defmodule PhotoFinish.Orders.OrderItem do
  use Ash.Resource,
    otp_app: :photo_finish,
    domain: PhotoFinish.Orders,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "order_items"
    repo PhotoFinish.Repo
  end

  actions do
    defaults [
      :read,
      :destroy,
      create: [
        :order_id,
        :event_product_id,
        :event_competitor_id,
        :quantity,
        :unit_price_cents,
        :line_total_cents,
        :fulfillment_status
      ],
      update: [
        :quantity,
        :unit_price_cents,
        :line_total_cents,
        :fulfillment_status
      ]
    ]
  end

  attributes do
    attribute :id, :string do
      primary_key? true
      allow_nil? false
      default &PhotoFinish.Id.order_item_id/0
      writable? false
    end

    attribute :quantity, :integer do
      public? true
      default 1
    end

    attribute :unit_price_cents, :integer do
      allow_nil? false
      public? true
    end

    attribute :line_total_cents, :integer do
      allow_nil? false
      public? true
    end

    attribute :fulfillment_status, :atom do
      public? true
      default :pending
      constraints one_of: [:pending, :fulfilled]
    end

    timestamps()
  end

  relationships do
    belongs_to :order, PhotoFinish.Orders.Order do
      public? true
      attribute_type :string
    end

    belongs_to :event_product, PhotoFinish.Orders.EventProduct do
      public? true
      attribute_type :string
    end

    belongs_to :event_competitor, PhotoFinish.Events.EventCompetitor do
      public? true
      attribute_type :string
    end
  end
end
