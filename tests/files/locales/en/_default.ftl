text = This is text
text-kwargs = This is text with args { $kwarg1 } { $kwarg2 }
-term1 = This is term1
-term2 = This is term2
text-args-term = This is text with args as term { -term1 } { -term2 }
-term1-with-args = This is term1 with args { $kwarg1 } { $kwarg2 }
-term2-with-args = This is term2 with args { $kwarg1 } { $kwarg2 }
message_reference = This is message_reference, uses as variable for `text-message_reference`
text-message_reference = This is text with another text { message_reference }
message_reference-args = This is message_reference with args { $kwarg1 } { $kwarg2 }, uses as variable for `text-message_reference-args`
text-message_reference-args = This is text with another text { message_reference-args }
text-args-term-args = This is text with args as term { -term1-with-args } { -term2-with-args }
text-selector =
    This is text with selector { $selector ->
        [1] Ok
        [2] Ok
       *[other] { $selector }, 🤔
    }
text-selector-selectors =
    This is text with selectors { $selector ->
        [1] Ok, { $selector }
        [2] Ok, { $selector }
       *[other] { $selector }, 🤔
    }
text-selector-kwargs =
    This is text with selector args { $selector ->
        [1] Ok, { $kwarg1 }
        [2] Ok, { $kwarg2 }
       *[other] 🤔
    }
-text-selector-reference-selector-kwargs-terms-term1 = This is term1 with args { $kwarg1 } { $kwarg2 }
text-selector-reference-selector-kwargs-terms-reference =
    This is text with selector args { $selector ->
        [1] Ok, { -text-selector-reference-selector-kwargs-terms-term1 }
        [2] Ok, { $kwarg2 }
       *[other] 🤔
    }
text-selector-reference-selector-kwargs-terms =
    This is text with selector args { $selector ->
        [1] Ok, { text-selector-reference-selector-kwargs-terms-reference }
        [2] Ok, { $kwarg1 }
       *[other] 🤔
    }
