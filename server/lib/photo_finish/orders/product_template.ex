defmodule PhotoFinish.Orders.ProductTemplate do
  use Ash.Resource,
    otp_app: :photo_finish,
    domain: PhotoFinish.Orders,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "product_templates"
    repo PhotoFinish.Repo
  end

  actions do
    defaults [
      :read,
      :destroy,
      create: [
        :product_type,
        :product_name,
        :product_size,
        :default_price_cents,
        :is_active,
        :display_order
      ],
      update: [
        :product_type,
        :product_name,
        :product_size,
        :default_price_cents,
        :is_active,
        :display_order
      ]
    ]
  end

  attributes do
    attribute :id, :string do
      primary_key? true
      allow_nil? false
      default &PhotoFinish.Id.product_template_id/0
      writable? false
    end

    attribute :product_type, :atom do
      allow_nil? false
      public? true
      constraints one_of: [:usb, :print, :collage, :custom_photo, :accessory]
    end

    attribute :product_name, :string do
      allow_nil? false
      public? true
    end

    attribute :product_size, :string do
      public? true
    end

    attribute :default_price_cents, :integer do
      allow_nil? false
      public? true
    end

    attribute :is_active, :boolean do
      public? true
      default true
    end

    attribute :display_order, :integer do
      public? true
      default 0
    end

    timestamps()
  end
end
