defmodule PhotoFinish.Orders.Order do
  use Ash.Resource,
    otp_app: :photo_finish,
    domain: PhotoFinish.Orders,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "orders"
    repo PhotoFinish.Repo
  end

  actions do
    defaults [
      :read,
      :destroy,
      create: [
        :event_id,
        :order_number,
        :customer_name,
        :customer_email,
        :customer_phone,
        :subtotal_cents,
        :tax_rate_basis_points,
        :tax_cents,
        :total_cents,
        :payment_status,
        :payment_reference,
        :notes
      ],
      update: [
        :customer_name,
        :customer_email,
        :customer_phone,
        :subtotal_cents,
        :tax_rate_basis_points,
        :tax_cents,
        :total_cents,
        :payment_status,
        :payment_reference,
        :notes
      ]
    ]
  end

  attributes do
    attribute :id, :string do
      primary_key? true
      allow_nil? false
      default &PhotoFinish.Id.order_id/0
      writable? false
    end

    attribute :order_number, :string do
      allow_nil? false
      public? true
    end

    attribute :customer_name, :string do
      allow_nil? false
      public? true
    end

    attribute :customer_email, :string do
      public? true
    end

    attribute :customer_phone, :string do
      public? true
    end

    attribute :subtotal_cents, :integer do
      allow_nil? false
      public? true
    end

    attribute :tax_rate_basis_points, :integer do
      allow_nil? false
      public? true
    end

    attribute :tax_cents, :integer do
      allow_nil? false
      public? true
    end

    attribute :total_cents, :integer do
      allow_nil? false
      public? true
    end

    attribute :payment_status, :atom do
      public? true
      default :pending
      constraints one_of: [:pending, :paid, :refunded]
    end

    attribute :payment_reference, :string do
      public? true
    end

    attribute :notes, :string do
      public? true
    end

    timestamps()
  end

  relationships do
    belongs_to :event, PhotoFinish.Events.Event do
      public? true
      attribute_type :string
    end

    has_many :order_items, PhotoFinish.Orders.OrderItem do
      public? true
    end
  end

  identities do
    identity :unique_order_number, [:order_number]
  end
end
