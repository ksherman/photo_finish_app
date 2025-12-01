defmodule PhotoFinish.Events.HierarchyNode do
  use Ash.Resource,
    otp_app: :photo_finish,
    domain: PhotoFinish.Events,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "hierarchy_nodes"
    repo PhotoFinish.Repo
  end

  actions do
    defaults [
      :read,
      :destroy,
      create: [:level_number, :name, :slug, :display_order, :metadata, :event_id, :parent_id],
      update: [:level_number, :name, :slug, :display_order, :metadata, :event_id, :parent_id]
    ]
  end

  attributes do
    uuid_primary_key :id

    attribute :level_number, :integer do
      allow_nil? false
      public? true
    end

    attribute :name, :string do
      allow_nil? false
      public? true
    end

    attribute :slug, :string do
      allow_nil? false
      public? true
    end

    attribute :display_order, :integer do
      public? true
      default 0
    end

    attribute :metadata, :map do
      public? true
    end

    timestamps()
  end

  relationships do
    belongs_to :event, PhotoFinish.Events.Event do
      public? true
    end

    belongs_to :parent, PhotoFinish.Events.HierarchyNode do
      public? true
    end

    has_many :children, PhotoFinish.Events.HierarchyNode do
      destination_attribute :parent_id
      public? true
    end

    has_many :competitors, PhotoFinish.Events.Competitor do
      destination_attribute :node_id
      public? true
    end

    has_many :photos, PhotoFinish.Photos.Photo do
      destination_attribute :node_id
      public? true
    end
  end
end