defmodule PhotoFinish.Orders do
  use Ash.Domain,
    otp_app: :photo_finish

  resources do
    resource PhotoFinish.Orders.ProductTemplate
    resource PhotoFinish.Orders.EventProduct
    resource PhotoFinish.Orders.Order
    resource PhotoFinish.Orders.OrderItem
  end
end
