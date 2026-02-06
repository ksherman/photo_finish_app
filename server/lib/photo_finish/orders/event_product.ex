defmodule PhotoFinish.Orders.EventProduct do
  use Ash.Resource,
    otp_app: :photo_finish,
    domain: PhotoFinish.Orders,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "event_products"
    repo PhotoFinish.Repo
  end

  actions do
    defaults [
      :read,
      :destroy,
      create: [
        :event_id,
        :product_template_id,
        :price_cents,
        :is_available
      ],
      update: [
        :price_cents,
        :is_available
      ]
    ]
  end

  attributes do
    attribute :id, :string do
      primary_key? true
      allow_nil? false
      default &PhotoFinish.Id.event_product_id/0
      writable? false
    end

    attribute :price_cents, :integer do
      allow_nil? false
      public? true
    end

    attribute :is_available, :boolean do
      public? true
      default true
    end

    timestamps()
  end

  relationships do
    belongs_to :event, PhotoFinish.Events.Event do
      public? true
      attribute_type :string
    end

    belongs_to :product_template, PhotoFinish.Orders.ProductTemplate do
      public? true
      attribute_type :string
    end
  end

  identities do
    identity :unique_event_product, [:event_id, :product_template_id]
  end
end
