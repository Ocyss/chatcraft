-- Deterministic fixture policy, independent of user-editable examples.
function chatcraft.filter_message(msg)
    if msg.chat_type == 'channel' and msg.text:find('空投', 1, true) then
        return { action = 'hide', reason = 'fixture spam' }
    end
    return 'allow'
end
