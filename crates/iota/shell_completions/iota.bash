_iota() {
    local i cur prev opts cmd
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    cmd=""
    opts=""

    for i in ${COMP_WORDS[@]}
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="iota"
                ;;
            iota,analyzer)
                cmd="iota__analyzer"
                ;;
            iota,bridge-committee-init)
                cmd="iota__bridge__committee__init"
                ;;
            iota,client)
                cmd="iota__client"
                ;;
            iota,console)
                cmd="iota__console"
                ;;
            iota,fire-drill)
                cmd="iota__fire__drill"
                ;;
            iota,generate-completions)
                cmd="iota__generate__completions"
                ;;
            iota,genesis)
                cmd="iota__genesis"
                ;;
            iota,genesis-ceremony)
                cmd="iota__genesis__ceremony"
                ;;
            iota,help)
                cmd="iota__help"
                ;;
            iota,keytool)
                cmd="iota__keytool"
                ;;
            iota,move)
                cmd="iota__move"
                ;;
            iota,start)
                cmd="iota__start"
                ;;
            iota,validator)
                cmd="iota__validator"
                ;;
            iota__client,active-address)
                cmd="iota__client__active__address"
                ;;
            iota__client,active-env)
                cmd="iota__client__active__env"
                ;;
            iota__client,addresses)
                cmd="iota__client__addresses"
                ;;
            iota__client,balance)
                cmd="iota__client__balance"
                ;;
            iota__client,call)
                cmd="iota__client__call"
                ;;
            iota__client,chain-identifier)
                cmd="iota__client__chain__identifier"
                ;;
            iota__client,dynamic-field)
                cmd="iota__client__dynamic__field"
                ;;
            iota__client,envs)
                cmd="iota__client__envs"
                ;;
            iota__client,execute-combined-signed-tx)
                cmd="iota__client__execute__combined__signed__tx"
                ;;
            iota__client,execute-signed-tx)
                cmd="iota__client__execute__signed__tx"
                ;;
            iota__client,faucet)
                cmd="iota__client__faucet"
                ;;
            iota__client,gas)
                cmd="iota__client__gas"
                ;;
            iota__client,help)
                cmd="iota__client__help"
                ;;
            iota__client,merge-coin)
                cmd="iota__client__merge__coin"
                ;;
            iota__client,new-address)
                cmd="iota__client__new__address"
                ;;
            iota__client,new-env)
                cmd="iota__client__new__env"
                ;;
            iota__client,object)
                cmd="iota__client__object"
                ;;
            iota__client,objects)
                cmd="iota__client__objects"
                ;;
            iota__client,pay)
                cmd="iota__client__pay"
                ;;
            iota__client,pay-all-iota)
                cmd="iota__client__pay__all__iota"
                ;;
            iota__client,pay-iota)
                cmd="iota__client__pay__iota"
                ;;
            iota__client,profile-transaction)
                cmd="iota__client__profile__transaction"
                ;;
            iota__client,ptb)
                cmd="iota__client__ptb"
                ;;
            iota__client,publish)
                cmd="iota__client__publish"
                ;;
            iota__client,replay-batch)
                cmd="iota__client__replay__batch"
                ;;
            iota__client,replay-checkpoint)
                cmd="iota__client__replay__checkpoint"
                ;;
            iota__client,replay-transaction)
                cmd="iota__client__replay__transaction"
                ;;
            iota__client,split-coin)
                cmd="iota__client__split__coin"
                ;;
            iota__client,switch)
                cmd="iota__client__switch"
                ;;
            iota__client,transfer)
                cmd="iota__client__transfer"
                ;;
            iota__client,transfer-iota)
                cmd="iota__client__transfer__iota"
                ;;
            iota__client,tx-block)
                cmd="iota__client__tx__block"
                ;;
            iota__client,upgrade)
                cmd="iota__client__upgrade"
                ;;
            iota__client,verify-bytecode-meter)
                cmd="iota__client__verify__bytecode__meter"
                ;;
            iota__client,verify-source)
                cmd="iota__client__verify__source"
                ;;
            iota__client__help,active-address)
                cmd="iota__client__help__active__address"
                ;;
            iota__client__help,active-env)
                cmd="iota__client__help__active__env"
                ;;
            iota__client__help,addresses)
                cmd="iota__client__help__addresses"
                ;;
            iota__client__help,balance)
                cmd="iota__client__help__balance"
                ;;
            iota__client__help,call)
                cmd="iota__client__help__call"
                ;;
            iota__client__help,chain-identifier)
                cmd="iota__client__help__chain__identifier"
                ;;
            iota__client__help,dynamic-field)
                cmd="iota__client__help__dynamic__field"
                ;;
            iota__client__help,envs)
                cmd="iota__client__help__envs"
                ;;
            iota__client__help,execute-combined-signed-tx)
                cmd="iota__client__help__execute__combined__signed__tx"
                ;;
            iota__client__help,execute-signed-tx)
                cmd="iota__client__help__execute__signed__tx"
                ;;
            iota__client__help,faucet)
                cmd="iota__client__help__faucet"
                ;;
            iota__client__help,gas)
                cmd="iota__client__help__gas"
                ;;
            iota__client__help,help)
                cmd="iota__client__help__help"
                ;;
            iota__client__help,merge-coin)
                cmd="iota__client__help__merge__coin"
                ;;
            iota__client__help,new-address)
                cmd="iota__client__help__new__address"
                ;;
            iota__client__help,new-env)
                cmd="iota__client__help__new__env"
                ;;
            iota__client__help,object)
                cmd="iota__client__help__object"
                ;;
            iota__client__help,objects)
                cmd="iota__client__help__objects"
                ;;
            iota__client__help,pay)
                cmd="iota__client__help__pay"
                ;;
            iota__client__help,pay-all-iota)
                cmd="iota__client__help__pay__all__iota"
                ;;
            iota__client__help,pay-iota)
                cmd="iota__client__help__pay__iota"
                ;;
            iota__client__help,profile-transaction)
                cmd="iota__client__help__profile__transaction"
                ;;
            iota__client__help,ptb)
                cmd="iota__client__help__ptb"
                ;;
            iota__client__help,publish)
                cmd="iota__client__help__publish"
                ;;
            iota__client__help,replay-batch)
                cmd="iota__client__help__replay__batch"
                ;;
            iota__client__help,replay-checkpoint)
                cmd="iota__client__help__replay__checkpoint"
                ;;
            iota__client__help,replay-transaction)
                cmd="iota__client__help__replay__transaction"
                ;;
            iota__client__help,split-coin)
                cmd="iota__client__help__split__coin"
                ;;
            iota__client__help,switch)
                cmd="iota__client__help__switch"
                ;;
            iota__client__help,transfer)
                cmd="iota__client__help__transfer"
                ;;
            iota__client__help,transfer-iota)
                cmd="iota__client__help__transfer__iota"
                ;;
            iota__client__help,tx-block)
                cmd="iota__client__help__tx__block"
                ;;
            iota__client__help,upgrade)
                cmd="iota__client__help__upgrade"
                ;;
            iota__client__help,verify-bytecode-meter)
                cmd="iota__client__help__verify__bytecode__meter"
                ;;
            iota__client__help,verify-source)
                cmd="iota__client__help__verify__source"
                ;;
            iota__fire__drill,help)
                cmd="iota__fire__drill__help"
                ;;
            iota__fire__drill,metadata-rotation)
                cmd="iota__fire__drill__metadata__rotation"
                ;;
            iota__fire__drill__help,help)
                cmd="iota__fire__drill__help__help"
                ;;
            iota__fire__drill__help,metadata-rotation)
                cmd="iota__fire__drill__help__metadata__rotation"
                ;;
            iota__genesis__ceremony,add-validator)
                cmd="iota__genesis__ceremony__add__validator"
                ;;
            iota__genesis__ceremony,build-unsigned-checkpoint)
                cmd="iota__genesis__ceremony__build__unsigned__checkpoint"
                ;;
            iota__genesis__ceremony,examine-genesis-checkpoint)
                cmd="iota__genesis__ceremony__examine__genesis__checkpoint"
                ;;
            iota__genesis__ceremony,finalize)
                cmd="iota__genesis__ceremony__finalize"
                ;;
            iota__genesis__ceremony,help)
                cmd="iota__genesis__ceremony__help"
                ;;
            iota__genesis__ceremony,init)
                cmd="iota__genesis__ceremony__init"
                ;;
            iota__genesis__ceremony,init-token-distribution-schedule)
                cmd="iota__genesis__ceremony__init__token__distribution__schedule"
                ;;
            iota__genesis__ceremony,list-validators)
                cmd="iota__genesis__ceremony__list__validators"
                ;;
            iota__genesis__ceremony,validate-state)
                cmd="iota__genesis__ceremony__validate__state"
                ;;
            iota__genesis__ceremony,verify-and-sign)
                cmd="iota__genesis__ceremony__verify__and__sign"
                ;;
            iota__genesis__ceremony__help,add-validator)
                cmd="iota__genesis__ceremony__help__add__validator"
                ;;
            iota__genesis__ceremony__help,build-unsigned-checkpoint)
                cmd="iota__genesis__ceremony__help__build__unsigned__checkpoint"
                ;;
            iota__genesis__ceremony__help,examine-genesis-checkpoint)
                cmd="iota__genesis__ceremony__help__examine__genesis__checkpoint"
                ;;
            iota__genesis__ceremony__help,finalize)
                cmd="iota__genesis__ceremony__help__finalize"
                ;;
            iota__genesis__ceremony__help,help)
                cmd="iota__genesis__ceremony__help__help"
                ;;
            iota__genesis__ceremony__help,init)
                cmd="iota__genesis__ceremony__help__init"
                ;;
            iota__genesis__ceremony__help,init-token-distribution-schedule)
                cmd="iota__genesis__ceremony__help__init__token__distribution__schedule"
                ;;
            iota__genesis__ceremony__help,list-validators)
                cmd="iota__genesis__ceremony__help__list__validators"
                ;;
            iota__genesis__ceremony__help,validate-state)
                cmd="iota__genesis__ceremony__help__validate__state"
                ;;
            iota__genesis__ceremony__help,verify-and-sign)
                cmd="iota__genesis__ceremony__help__verify__and__sign"
                ;;
            iota__help,analyzer)
                cmd="iota__help__analyzer"
                ;;
            iota__help,bridge-committee-init)
                cmd="iota__help__bridge__committee__init"
                ;;
            iota__help,client)
                cmd="iota__help__client"
                ;;
            iota__help,console)
                cmd="iota__help__console"
                ;;
            iota__help,fire-drill)
                cmd="iota__help__fire__drill"
                ;;
            iota__help,generate-completions)
                cmd="iota__help__generate__completions"
                ;;
            iota__help,genesis)
                cmd="iota__help__genesis"
                ;;
            iota__help,genesis-ceremony)
                cmd="iota__help__genesis__ceremony"
                ;;
            iota__help,help)
                cmd="iota__help__help"
                ;;
            iota__help,keytool)
                cmd="iota__help__keytool"
                ;;
            iota__help,move)
                cmd="iota__help__move"
                ;;
            iota__help,start)
                cmd="iota__help__start"
                ;;
            iota__help,validator)
                cmd="iota__help__validator"
                ;;
            iota__help__client,active-address)
                cmd="iota__help__client__active__address"
                ;;
            iota__help__client,active-env)
                cmd="iota__help__client__active__env"
                ;;
            iota__help__client,addresses)
                cmd="iota__help__client__addresses"
                ;;
            iota__help__client,balance)
                cmd="iota__help__client__balance"
                ;;
            iota__help__client,call)
                cmd="iota__help__client__call"
                ;;
            iota__help__client,chain-identifier)
                cmd="iota__help__client__chain__identifier"
                ;;
            iota__help__client,dynamic-field)
                cmd="iota__help__client__dynamic__field"
                ;;
            iota__help__client,envs)
                cmd="iota__help__client__envs"
                ;;
            iota__help__client,execute-combined-signed-tx)
                cmd="iota__help__client__execute__combined__signed__tx"
                ;;
            iota__help__client,execute-signed-tx)
                cmd="iota__help__client__execute__signed__tx"
                ;;
            iota__help__client,faucet)
                cmd="iota__help__client__faucet"
                ;;
            iota__help__client,gas)
                cmd="iota__help__client__gas"
                ;;
            iota__help__client,merge-coin)
                cmd="iota__help__client__merge__coin"
                ;;
            iota__help__client,new-address)
                cmd="iota__help__client__new__address"
                ;;
            iota__help__client,new-env)
                cmd="iota__help__client__new__env"
                ;;
            iota__help__client,object)
                cmd="iota__help__client__object"
                ;;
            iota__help__client,objects)
                cmd="iota__help__client__objects"
                ;;
            iota__help__client,pay)
                cmd="iota__help__client__pay"
                ;;
            iota__help__client,pay-all-iota)
                cmd="iota__help__client__pay__all__iota"
                ;;
            iota__help__client,pay-iota)
                cmd="iota__help__client__pay__iota"
                ;;
            iota__help__client,profile-transaction)
                cmd="iota__help__client__profile__transaction"
                ;;
            iota__help__client,ptb)
                cmd="iota__help__client__ptb"
                ;;
            iota__help__client,publish)
                cmd="iota__help__client__publish"
                ;;
            iota__help__client,replay-batch)
                cmd="iota__help__client__replay__batch"
                ;;
            iota__help__client,replay-checkpoint)
                cmd="iota__help__client__replay__checkpoint"
                ;;
            iota__help__client,replay-transaction)
                cmd="iota__help__client__replay__transaction"
                ;;
            iota__help__client,split-coin)
                cmd="iota__help__client__split__coin"
                ;;
            iota__help__client,switch)
                cmd="iota__help__client__switch"
                ;;
            iota__help__client,transfer)
                cmd="iota__help__client__transfer"
                ;;
            iota__help__client,transfer-iota)
                cmd="iota__help__client__transfer__iota"
                ;;
            iota__help__client,tx-block)
                cmd="iota__help__client__tx__block"
                ;;
            iota__help__client,upgrade)
                cmd="iota__help__client__upgrade"
                ;;
            iota__help__client,verify-bytecode-meter)
                cmd="iota__help__client__verify__bytecode__meter"
                ;;
            iota__help__client,verify-source)
                cmd="iota__help__client__verify__source"
                ;;
            iota__help__fire__drill,metadata-rotation)
                cmd="iota__help__fire__drill__metadata__rotation"
                ;;
            iota__help__genesis__ceremony,add-validator)
                cmd="iota__help__genesis__ceremony__add__validator"
                ;;
            iota__help__genesis__ceremony,build-unsigned-checkpoint)
                cmd="iota__help__genesis__ceremony__build__unsigned__checkpoint"
                ;;
            iota__help__genesis__ceremony,examine-genesis-checkpoint)
                cmd="iota__help__genesis__ceremony__examine__genesis__checkpoint"
                ;;
            iota__help__genesis__ceremony,finalize)
                cmd="iota__help__genesis__ceremony__finalize"
                ;;
            iota__help__genesis__ceremony,init)
                cmd="iota__help__genesis__ceremony__init"
                ;;
            iota__help__genesis__ceremony,init-token-distribution-schedule)
                cmd="iota__help__genesis__ceremony__init__token__distribution__schedule"
                ;;
            iota__help__genesis__ceremony,list-validators)
                cmd="iota__help__genesis__ceremony__list__validators"
                ;;
            iota__help__genesis__ceremony,validate-state)
                cmd="iota__help__genesis__ceremony__validate__state"
                ;;
            iota__help__genesis__ceremony,verify-and-sign)
                cmd="iota__help__genesis__ceremony__verify__and__sign"
                ;;
            iota__help__keytool,convert)
                cmd="iota__help__keytool__convert"
                ;;
            iota__help__keytool,decode-multi-sig)
                cmd="iota__help__keytool__decode__multi__sig"
                ;;
            iota__help__keytool,decode-or-verify-tx)
                cmd="iota__help__keytool__decode__or__verify__tx"
                ;;
            iota__help__keytool,export)
                cmd="iota__help__keytool__export"
                ;;
            iota__help__keytool,generate)
                cmd="iota__help__keytool__generate"
                ;;
            iota__help__keytool,import)
                cmd="iota__help__keytool__import"
                ;;
            iota__help__keytool,list)
                cmd="iota__help__keytool__list"
                ;;
            iota__help__keytool,multi-sig-address)
                cmd="iota__help__keytool__multi__sig__address"
                ;;
            iota__help__keytool,multi-sig-combine-partial-sig)
                cmd="iota__help__keytool__multi__sig__combine__partial__sig"
                ;;
            iota__help__keytool,show)
                cmd="iota__help__keytool__show"
                ;;
            iota__help__keytool,sign)
                cmd="iota__help__keytool__sign"
                ;;
            iota__help__keytool,sign-kms)
                cmd="iota__help__keytool__sign__kms"
                ;;
            iota__help__keytool,update-alias)
                cmd="iota__help__keytool__update__alias"
                ;;
            iota__help__move,build)
                cmd="iota__help__move__build"
                ;;
            iota__help__move,coverage)
                cmd="iota__help__move__coverage"
                ;;
            iota__help__move,disassemble)
                cmd="iota__help__move__disassemble"
                ;;
            iota__help__move,manage-package)
                cmd="iota__help__move__manage__package"
                ;;
            iota__help__move,migrate)
                cmd="iota__help__move__migrate"
                ;;
            iota__help__move,new)
                cmd="iota__help__move__new"
                ;;
            iota__help__move,test)
                cmd="iota__help__move__test"
                ;;
            iota__help__move__coverage,bytecode)
                cmd="iota__help__move__coverage__bytecode"
                ;;
            iota__help__move__coverage,source)
                cmd="iota__help__move__coverage__source"
                ;;
            iota__help__move__coverage,summary)
                cmd="iota__help__move__coverage__summary"
                ;;
            iota__help__validator,become-candidate)
                cmd="iota__help__validator__become__candidate"
                ;;
            iota__help__validator,display-gas-price-update-raw-txn)
                cmd="iota__help__validator__display__gas__price__update__raw__txn"
                ;;
            iota__help__validator,display-metadata)
                cmd="iota__help__validator__display__metadata"
                ;;
            iota__help__validator,join-committee)
                cmd="iota__help__validator__join__committee"
                ;;
            iota__help__validator,leave-committee)
                cmd="iota__help__validator__leave__committee"
                ;;
            iota__help__validator,list)
                cmd="iota__help__validator__list"
                ;;
            iota__help__validator,make-validator-info)
                cmd="iota__help__validator__make__validator__info"
                ;;
            iota__help__validator,register-bridge-committee)
                cmd="iota__help__validator__register__bridge__committee"
                ;;
            iota__help__validator,report-validator)
                cmd="iota__help__validator__report__validator"
                ;;
            iota__help__validator,serialize-payload-pop)
                cmd="iota__help__validator__serialize__payload__pop"
                ;;
            iota__help__validator,update-bridge-committee-node-url)
                cmd="iota__help__validator__update__bridge__committee__node__url"
                ;;
            iota__help__validator,update-gas-price)
                cmd="iota__help__validator__update__gas__price"
                ;;
            iota__help__validator,update-metadata)
                cmd="iota__help__validator__update__metadata"
                ;;
            iota__help__validator__update__metadata,authority-pub-key)
                cmd="iota__help__validator__update__metadata__authority__pub__key"
                ;;
            iota__help__validator__update__metadata,description)
                cmd="iota__help__validator__update__metadata__description"
                ;;
            iota__help__validator__update__metadata,image-url)
                cmd="iota__help__validator__update__metadata__image__url"
                ;;
            iota__help__validator__update__metadata,name)
                cmd="iota__help__validator__update__metadata__name"
                ;;
            iota__help__validator__update__metadata,network-address)
                cmd="iota__help__validator__update__metadata__network__address"
                ;;
            iota__help__validator__update__metadata,network-pub-key)
                cmd="iota__help__validator__update__metadata__network__pub__key"
                ;;
            iota__help__validator__update__metadata,p2p-address)
                cmd="iota__help__validator__update__metadata__p2p__address"
                ;;
            iota__help__validator__update__metadata,primary-address)
                cmd="iota__help__validator__update__metadata__primary__address"
                ;;
            iota__help__validator__update__metadata,project-url)
                cmd="iota__help__validator__update__metadata__project__url"
                ;;
            iota__help__validator__update__metadata,protocol-pub-key)
                cmd="iota__help__validator__update__metadata__protocol__pub__key"
                ;;
            iota__keytool,convert)
                cmd="iota__keytool__convert"
                ;;
            iota__keytool,decode-multi-sig)
                cmd="iota__keytool__decode__multi__sig"
                ;;
            iota__keytool,decode-or-verify-tx)
                cmd="iota__keytool__decode__or__verify__tx"
                ;;
            iota__keytool,export)
                cmd="iota__keytool__export"
                ;;
            iota__keytool,generate)
                cmd="iota__keytool__generate"
                ;;
            iota__keytool,help)
                cmd="iota__keytool__help"
                ;;
            iota__keytool,import)
                cmd="iota__keytool__import"
                ;;
            iota__keytool,list)
                cmd="iota__keytool__list"
                ;;
            iota__keytool,multi-sig-address)
                cmd="iota__keytool__multi__sig__address"
                ;;
            iota__keytool,multi-sig-combine-partial-sig)
                cmd="iota__keytool__multi__sig__combine__partial__sig"
                ;;
            iota__keytool,show)
                cmd="iota__keytool__show"
                ;;
            iota__keytool,sign)
                cmd="iota__keytool__sign"
                ;;
            iota__keytool,sign-kms)
                cmd="iota__keytool__sign__kms"
                ;;
            iota__keytool,update-alias)
                cmd="iota__keytool__update__alias"
                ;;
            iota__keytool__help,convert)
                cmd="iota__keytool__help__convert"
                ;;
            iota__keytool__help,decode-multi-sig)
                cmd="iota__keytool__help__decode__multi__sig"
                ;;
            iota__keytool__help,decode-or-verify-tx)
                cmd="iota__keytool__help__decode__or__verify__tx"
                ;;
            iota__keytool__help,export)
                cmd="iota__keytool__help__export"
                ;;
            iota__keytool__help,generate)
                cmd="iota__keytool__help__generate"
                ;;
            iota__keytool__help,help)
                cmd="iota__keytool__help__help"
                ;;
            iota__keytool__help,import)
                cmd="iota__keytool__help__import"
                ;;
            iota__keytool__help,list)
                cmd="iota__keytool__help__list"
                ;;
            iota__keytool__help,multi-sig-address)
                cmd="iota__keytool__help__multi__sig__address"
                ;;
            iota__keytool__help,multi-sig-combine-partial-sig)
                cmd="iota__keytool__help__multi__sig__combine__partial__sig"
                ;;
            iota__keytool__help,show)
                cmd="iota__keytool__help__show"
                ;;
            iota__keytool__help,sign)
                cmd="iota__keytool__help__sign"
                ;;
            iota__keytool__help,sign-kms)
                cmd="iota__keytool__help__sign__kms"
                ;;
            iota__keytool__help,update-alias)
                cmd="iota__keytool__help__update__alias"
                ;;
            iota__move,build)
                cmd="iota__move__build"
                ;;
            iota__move,coverage)
                cmd="iota__move__coverage"
                ;;
            iota__move,disassemble)
                cmd="iota__move__disassemble"
                ;;
            iota__move,help)
                cmd="iota__move__help"
                ;;
            iota__move,manage-package)
                cmd="iota__move__manage__package"
                ;;
            iota__move,migrate)
                cmd="iota__move__migrate"
                ;;
            iota__move,new)
                cmd="iota__move__new"
                ;;
            iota__move,test)
                cmd="iota__move__test"
                ;;
            iota__move__coverage,bytecode)
                cmd="iota__move__coverage__bytecode"
                ;;
            iota__move__coverage,help)
                cmd="iota__move__coverage__help"
                ;;
            iota__move__coverage,source)
                cmd="iota__move__coverage__source"
                ;;
            iota__move__coverage,summary)
                cmd="iota__move__coverage__summary"
                ;;
            iota__move__coverage__help,bytecode)
                cmd="iota__move__coverage__help__bytecode"
                ;;
            iota__move__coverage__help,help)
                cmd="iota__move__coverage__help__help"
                ;;
            iota__move__coverage__help,source)
                cmd="iota__move__coverage__help__source"
                ;;
            iota__move__coverage__help,summary)
                cmd="iota__move__coverage__help__summary"
                ;;
            iota__move__help,build)
                cmd="iota__move__help__build"
                ;;
            iota__move__help,coverage)
                cmd="iota__move__help__coverage"
                ;;
            iota__move__help,disassemble)
                cmd="iota__move__help__disassemble"
                ;;
            iota__move__help,help)
                cmd="iota__move__help__help"
                ;;
            iota__move__help,manage-package)
                cmd="iota__move__help__manage__package"
                ;;
            iota__move__help,migrate)
                cmd="iota__move__help__migrate"
                ;;
            iota__move__help,new)
                cmd="iota__move__help__new"
                ;;
            iota__move__help,test)
                cmd="iota__move__help__test"
                ;;
            iota__move__help__coverage,bytecode)
                cmd="iota__move__help__coverage__bytecode"
                ;;
            iota__move__help__coverage,source)
                cmd="iota__move__help__coverage__source"
                ;;
            iota__move__help__coverage,summary)
                cmd="iota__move__help__coverage__summary"
                ;;
            iota__validator,become-candidate)
                cmd="iota__validator__become__candidate"
                ;;
            iota__validator,display-gas-price-update-raw-txn)
                cmd="iota__validator__display__gas__price__update__raw__txn"
                ;;
            iota__validator,display-metadata)
                cmd="iota__validator__display__metadata"
                ;;
            iota__validator,help)
                cmd="iota__validator__help"
                ;;
            iota__validator,join-committee)
                cmd="iota__validator__join__committee"
                ;;
            iota__validator,leave-committee)
                cmd="iota__validator__leave__committee"
                ;;
            iota__validator,list)
                cmd="iota__validator__list"
                ;;
            iota__validator,make-validator-info)
                cmd="iota__validator__make__validator__info"
                ;;
            iota__validator,register-bridge-committee)
                cmd="iota__validator__register__bridge__committee"
                ;;
            iota__validator,report-validator)
                cmd="iota__validator__report__validator"
                ;;
            iota__validator,serialize-payload-pop)
                cmd="iota__validator__serialize__payload__pop"
                ;;
            iota__validator,update-bridge-committee-node-url)
                cmd="iota__validator__update__bridge__committee__node__url"
                ;;
            iota__validator,update-gas-price)
                cmd="iota__validator__update__gas__price"
                ;;
            iota__validator,update-metadata)
                cmd="iota__validator__update__metadata"
                ;;
            iota__validator__help,become-candidate)
                cmd="iota__validator__help__become__candidate"
                ;;
            iota__validator__help,display-gas-price-update-raw-txn)
                cmd="iota__validator__help__display__gas__price__update__raw__txn"
                ;;
            iota__validator__help,display-metadata)
                cmd="iota__validator__help__display__metadata"
                ;;
            iota__validator__help,help)
                cmd="iota__validator__help__help"
                ;;
            iota__validator__help,join-committee)
                cmd="iota__validator__help__join__committee"
                ;;
            iota__validator__help,leave-committee)
                cmd="iota__validator__help__leave__committee"
                ;;
            iota__validator__help,list)
                cmd="iota__validator__help__list"
                ;;
            iota__validator__help,make-validator-info)
                cmd="iota__validator__help__make__validator__info"
                ;;
            iota__validator__help,register-bridge-committee)
                cmd="iota__validator__help__register__bridge__committee"
                ;;
            iota__validator__help,report-validator)
                cmd="iota__validator__help__report__validator"
                ;;
            iota__validator__help,serialize-payload-pop)
                cmd="iota__validator__help__serialize__payload__pop"
                ;;
            iota__validator__help,update-bridge-committee-node-url)
                cmd="iota__validator__help__update__bridge__committee__node__url"
                ;;
            iota__validator__help,update-gas-price)
                cmd="iota__validator__help__update__gas__price"
                ;;
            iota__validator__help,update-metadata)
                cmd="iota__validator__help__update__metadata"
                ;;
            iota__validator__help__update__metadata,authority-pub-key)
                cmd="iota__validator__help__update__metadata__authority__pub__key"
                ;;
            iota__validator__help__update__metadata,description)
                cmd="iota__validator__help__update__metadata__description"
                ;;
            iota__validator__help__update__metadata,image-url)
                cmd="iota__validator__help__update__metadata__image__url"
                ;;
            iota__validator__help__update__metadata,name)
                cmd="iota__validator__help__update__metadata__name"
                ;;
            iota__validator__help__update__metadata,network-address)
                cmd="iota__validator__help__update__metadata__network__address"
                ;;
            iota__validator__help__update__metadata,network-pub-key)
                cmd="iota__validator__help__update__metadata__network__pub__key"
                ;;
            iota__validator__help__update__metadata,p2p-address)
                cmd="iota__validator__help__update__metadata__p2p__address"
                ;;
            iota__validator__help__update__metadata,primary-address)
                cmd="iota__validator__help__update__metadata__primary__address"
                ;;
            iota__validator__help__update__metadata,project-url)
                cmd="iota__validator__help__update__metadata__project__url"
                ;;
            iota__validator__help__update__metadata,protocol-pub-key)
                cmd="iota__validator__help__update__metadata__protocol__pub__key"
                ;;
            iota__validator__update__metadata,authority-pub-key)
                cmd="iota__validator__update__metadata__authority__pub__key"
                ;;
            iota__validator__update__metadata,description)
                cmd="iota__validator__update__metadata__description"
                ;;
            iota__validator__update__metadata,help)
                cmd="iota__validator__update__metadata__help"
                ;;
            iota__validator__update__metadata,image-url)
                cmd="iota__validator__update__metadata__image__url"
                ;;
            iota__validator__update__metadata,name)
                cmd="iota__validator__update__metadata__name"
                ;;
            iota__validator__update__metadata,network-address)
                cmd="iota__validator__update__metadata__network__address"
                ;;
            iota__validator__update__metadata,network-pub-key)
                cmd="iota__validator__update__metadata__network__pub__key"
                ;;
            iota__validator__update__metadata,p2p-address)
                cmd="iota__validator__update__metadata__p2p__address"
                ;;
            iota__validator__update__metadata,primary-address)
                cmd="iota__validator__update__metadata__primary__address"
                ;;
            iota__validator__update__metadata,project-url)
                cmd="iota__validator__update__metadata__project__url"
                ;;
            iota__validator__update__metadata,protocol-pub-key)
                cmd="iota__validator__update__metadata__protocol__pub__key"
                ;;
            iota__validator__update__metadata__help,authority-pub-key)
                cmd="iota__validator__update__metadata__help__authority__pub__key"
                ;;
            iota__validator__update__metadata__help,description)
                cmd="iota__validator__update__metadata__help__description"
                ;;
            iota__validator__update__metadata__help,help)
                cmd="iota__validator__update__metadata__help__help"
                ;;
            iota__validator__update__metadata__help,image-url)
                cmd="iota__validator__update__metadata__help__image__url"
                ;;
            iota__validator__update__metadata__help,name)
                cmd="iota__validator__update__metadata__help__name"
                ;;
            iota__validator__update__metadata__help,network-address)
                cmd="iota__validator__update__metadata__help__network__address"
                ;;
            iota__validator__update__metadata__help,network-pub-key)
                cmd="iota__validator__update__metadata__help__network__pub__key"
                ;;
            iota__validator__update__metadata__help,p2p-address)
                cmd="iota__validator__update__metadata__help__p2p__address"
                ;;
            iota__validator__update__metadata__help,primary-address)
                cmd="iota__validator__update__metadata__help__primary__address"
                ;;
            iota__validator__update__metadata__help,project-url)
                cmd="iota__validator__update__metadata__help__project__url"
                ;;
            iota__validator__update__metadata__help,protocol-pub-key)
                cmd="iota__validator__update__metadata__help__protocol__pub__key"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        iota)
            opts="-h --help start genesis genesis-ceremony keytool console client validator move bridge-committee-init fire-drill analyzer generate-completions help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__analyzer)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__bridge__committee__init)
            opts="-h --network.config --client.config --bridge_committee.config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --network.config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --client.config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --bridge_committee.config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client)
            opts="-y -h --client.config --json --yes --help active-address active-env addresses balance call chain-identifier dynamic-field envs execute-signed-tx execute-combined-signed-tx faucet gas merge-coin new-address new-env object objects pay pay-all-iota pay-iota ptb publish split-coin switch tx-block transfer transfer-iota upgrade verify-bytecode-meter verify-source profile-transaction replay-transaction replay-batch replay-checkpoint help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --client.config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__active__address)
            opts="-h --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__active__env)
            opts="-h --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__addresses)
            opts="-s -h --sort-by-alias --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__balance)
            opts="-h --coin-type --with-coins --json --help [ADDRESS]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --coin-type)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__call)
            opts="-h --package --module --function --type-args --args --gas --gas-budget --dry-run --serialize-unsigned-transaction --serialize-signed-transaction --emit --json --help [GAS_PRICE]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --package)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --module)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --function)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --type-args)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --args)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --emit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__chain__identifier)
            opts="-h --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__dynamic__field)
            opts="-h --cursor --limit --json --help <object_id>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --cursor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --limit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__envs)
            opts="-h --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__execute__combined__signed__tx)
            opts="-h --signed-tx-bytes --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --signed-tx-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__execute__signed__tx)
            opts="-h --tx-bytes --signatures --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --tx-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --signatures)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__faucet)
            opts="-h --address --url --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__gas)
            opts="-h --json --help [owner_address]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help)
            opts="active-address active-env addresses balance call chain-identifier dynamic-field envs execute-signed-tx execute-combined-signed-tx faucet gas merge-coin new-address new-env object objects pay pay-all-iota pay-iota ptb publish split-coin switch tx-block transfer transfer-iota upgrade verify-bytecode-meter verify-source profile-transaction replay-transaction replay-batch replay-checkpoint help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__active__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__active__env)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__addresses)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__balance)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__call)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__chain__identifier)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__dynamic__field)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__envs)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__execute__combined__signed__tx)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__execute__signed__tx)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__faucet)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__gas)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__merge__coin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__new__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__new__env)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__object)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__objects)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__pay)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__pay__all__iota)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__pay__iota)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__profile__transaction)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__ptb)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__publish)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__replay__batch)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__replay__checkpoint)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__replay__transaction)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__split__coin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__switch)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__transfer)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__transfer__iota)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__tx__block)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__upgrade)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__verify__bytecode__meter)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__help__verify__source)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__merge__coin)
            opts="-h --primary-coin --coin-to-merge --gas --gas-budget --dry-run --serialize-unsigned-transaction --serialize-signed-transaction --emit --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --primary-coin)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --coin-to-merge)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --emit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__new__address)
            opts="-h --json --help <KEY_SCHEME> [ALIAS] [WORD_LENGTH] [DERIVATION_PATH]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__new__env)
            opts="-h --alias --rpc --ws --basic-auth --faucet --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --alias)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --rpc)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --ws)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --basic-auth)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --faucet)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__object)
            opts="-h --bcs --json --help <object_id>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__objects)
            opts="-h --json --help [owner_address]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__pay)
            opts="-h --input-coins --recipients --amounts --gas --gas-budget --dry-run --serialize-unsigned-transaction --serialize-signed-transaction --emit --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --input-coins)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --recipients)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --amounts)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --emit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__pay__all__iota)
            opts="-h --input-coins --recipient --gas-budget --dry-run --serialize-unsigned-transaction --serialize-signed-transaction --emit --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --input-coins)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --recipient)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --emit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__pay__iota)
            opts="-h --input-coins --recipients --amounts --gas-budget --dry-run --serialize-unsigned-transaction --serialize-signed-transaction --emit --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --input-coins)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --recipients)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --amounts)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --emit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__profile__transaction)
            opts="-t -p -h --tx-digest --profile-output --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --tx-digest)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -t)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --profile-output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__ptb)
            opts="--json [ARGS]..."
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__publish)
            opts="-d -h --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --gas --gas-budget --dry-run --serialize-unsigned-transaction --serialize-signed-transaction --emit --skip-dependency-verification --with-unpublished-dependencies --json --help [package_path]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --emit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__replay__batch)
            opts="-p -t -h --path --terminate-early --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__replay__checkpoint)
            opts="-s -e -t -h --start --end --terminate-early --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --start)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -s)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --end)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -e)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__replay__transaction)
            opts="-t -e -p -h --tx-digest --gas-info --ptb-info --executor-version --protocol-version --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --tx-digest)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -t)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --executor-version)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -e)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --protocol-version)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__split__coin)
            opts="-h --coin-id --amounts --count --gas --gas-budget --dry-run --serialize-unsigned-transaction --serialize-signed-transaction --emit --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --coin-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --amounts)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --count)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --emit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__switch)
            opts="-h --address --env --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --env)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__transfer)
            opts="-h --to --object-id --gas --gas-budget --dry-run --serialize-unsigned-transaction --serialize-signed-transaction --emit --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --to)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --object-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --emit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__transfer__iota)
            opts="-h --to --iota-coin-object-id --amount --gas-budget --dry-run --serialize-unsigned-transaction --serialize-signed-transaction --emit --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --to)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --iota-coin-object-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --amount)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --emit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__tx__block)
            opts="-h --json --help <digest>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__upgrade)
            opts="-d -h --upgrade-capability --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --gas --gas-budget --dry-run --serialize-unsigned-transaction --serialize-signed-transaction --emit --skip-dependency-verification --with-unpublished-dependencies --json --help [package_path]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --upgrade-capability)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --emit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__verify__bytecode__meter)
            opts="-d -h --package --protocol-version --module --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --package)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --protocol-version)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --module)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__client__verify__source)
            opts="-d -h --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --verify-deps --skip-source --address-override --json --help [package_path]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --address-override)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__console)
            opts="-h --client.config --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --client.config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__fire__drill)
            opts="-h --help metadata-rotation help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__fire__drill__help)
            opts="metadata-rotation help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__fire__drill__help__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__fire__drill__help__metadata__rotation)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__fire__drill__metadata__rotation)
            opts="-h --iota-node-config-path --account-key-path --fullnode-rpc-url --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --iota-node-config-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --account-key-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --fullnode-rpc-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__generate__completions)
            opts="-s -o -h --shell --out-dir --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --shell)
                    COMPREPLY=($(compgen -W "bash elvish fish nushell powershell zsh" -- "${cur}"))
                    return 0
                    ;;
                -s)
                    COMPREPLY=($(compgen -W "bash elvish fish nushell powershell zsh" -- "${cur}"))
                    return 0
                    ;;
                --out-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -o)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis)
            opts="-f -h --from-config --write-config --working-dir --force --epoch-duration-ms --benchmark-ips --with-faucet --num-validators --local-migration-snapshots --remote-migration-snapshots --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --from-config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --write-config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --working-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --epoch-duration-ms)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --benchmark-ips)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --num-validators)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --local-migration-snapshots)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --remote-migration-snapshots)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony)
            opts="-h --path --protocol-version --help init validate-state add-validator init-token-distribution-schedule list-validators build-unsigned-checkpoint examine-genesis-checkpoint verify-and-sign finalize help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --protocol-version)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__add__validator)
            opts="-h --name --authority-key-file --protocol-key-file --account-key-file --network-key-file --network-address --p2p-address --primary-address --description --image-url --project-url --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --name)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --authority-key-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --protocol-key-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --account-key-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --network-key-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --network-address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --p2p-address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --primary-address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --description)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --image-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --project-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__build__unsigned__checkpoint)
            opts="-h --local-migration-snapshots --remote-migration-snapshots --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --local-migration-snapshots)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --remote-migration-snapshots)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__examine__genesis__checkpoint)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__finalize)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__help)
            opts="init validate-state add-validator init-token-distribution-schedule list-validators build-unsigned-checkpoint examine-genesis-checkpoint verify-and-sign finalize help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__help__add__validator)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__help__build__unsigned__checkpoint)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__help__examine__genesis__checkpoint)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__help__finalize)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__help__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__help__init)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__help__init__token__distribution__schedule)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__help__list__validators)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__help__validate__state)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__help__verify__and__sign)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__init)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__init__token__distribution__schedule)
            opts="-h --token-allocations-path --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --token-allocations-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__list__validators)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__validate__state)
            opts="-h --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__genesis__ceremony__verify__and__sign)
            opts="-h --key-file --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --key-file)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help)
            opts="start genesis genesis-ceremony keytool console client validator move bridge-committee-init fire-drill analyzer generate-completions help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__analyzer)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__bridge__committee__init)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client)
            opts="active-address active-env addresses balance call chain-identifier dynamic-field envs execute-signed-tx execute-combined-signed-tx faucet gas merge-coin new-address new-env object objects pay pay-all-iota pay-iota ptb publish split-coin switch tx-block transfer transfer-iota upgrade verify-bytecode-meter verify-source profile-transaction replay-transaction replay-batch replay-checkpoint"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__active__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__active__env)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__addresses)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__balance)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__call)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__chain__identifier)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__dynamic__field)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__envs)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__execute__combined__signed__tx)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__execute__signed__tx)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__faucet)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__gas)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__merge__coin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__new__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__new__env)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__object)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__objects)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__pay)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__pay__all__iota)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__pay__iota)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__profile__transaction)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__ptb)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__publish)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__replay__batch)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__replay__checkpoint)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__replay__transaction)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__split__coin)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__switch)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__transfer)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__transfer__iota)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__tx__block)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__upgrade)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__verify__bytecode__meter)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__client__verify__source)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__console)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__fire__drill)
            opts="metadata-rotation"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__fire__drill__metadata__rotation)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__generate__completions)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__genesis)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__genesis__ceremony)
            opts="init validate-state add-validator init-token-distribution-schedule list-validators build-unsigned-checkpoint examine-genesis-checkpoint verify-and-sign finalize"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__genesis__ceremony__add__validator)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__genesis__ceremony__build__unsigned__checkpoint)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__genesis__ceremony__examine__genesis__checkpoint)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__genesis__ceremony__finalize)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__genesis__ceremony__init)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__genesis__ceremony__init__token__distribution__schedule)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__genesis__ceremony__list__validators)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__genesis__ceremony__validate__state)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__genesis__ceremony__verify__and__sign)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool)
            opts="update-alias convert decode-or-verify-tx decode-multi-sig generate import export list multi-sig-address multi-sig-combine-partial-sig show sign sign-kms"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__convert)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__decode__multi__sig)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__decode__or__verify__tx)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__export)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__generate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__import)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__multi__sig__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__multi__sig__combine__partial__sig)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__show)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__sign)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__sign__kms)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__keytool__update__alias)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__move)
            opts="build coverage disassemble manage-package migrate new test"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__move__build)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__move__coverage)
            opts="summary source bytecode"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__move__coverage__bytecode)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__move__coverage__source)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__move__coverage__summary)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__move__disassemble)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__move__manage__package)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__move__migrate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__move__new)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__move__test)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__start)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator)
            opts="make-validator-info become-candidate join-committee leave-committee display-metadata update-metadata update-gas-price report-validator serialize-payload-pop display-gas-price-update-raw-txn register-bridge-committee update-bridge-committee-node-url list"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__become__candidate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__display__gas__price__update__raw__txn)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__display__metadata)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__join__committee)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__leave__committee)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__make__validator__info)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__register__bridge__committee)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__report__validator)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__serialize__payload__pop)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__bridge__committee__node__url)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__gas__price)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__metadata)
            opts="name description image-url project-url network-address primary-address p2p-address network-pub-key protocol-pub-key authority-pub-key"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__metadata__authority__pub__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__metadata__description)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__metadata__image__url)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__metadata__name)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__metadata__network__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__metadata__network__pub__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__metadata__p2p__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__metadata__primary__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__metadata__project__url)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__help__validator__update__metadata__protocol__pub__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool)
            opts="-h --keystore-path --json --help update-alias convert decode-or-verify-tx decode-multi-sig generate import export list multi-sig-address multi-sig-combine-partial-sig show sign sign-kms help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --keystore-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__convert)
            opts="-h --json --help <VALUE>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__decode__multi__sig)
            opts="-h --multisig --tx-bytes --cur-epoch --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --multisig)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --tx-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --cur-epoch)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__decode__or__verify__tx)
            opts="-h --tx-bytes --sig --cur-epoch --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --tx-bytes)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --sig)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --cur-epoch)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__export)
            opts="-h --key-identity --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --key-identity)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__generate)
            opts="-h --json --help <KEY_SCHEME> [DERIVATION_PATH] [WORD_LENGTH]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help)
            opts="update-alias convert decode-or-verify-tx decode-multi-sig generate import export list multi-sig-address multi-sig-combine-partial-sig show sign sign-kms help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__convert)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__decode__multi__sig)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__decode__or__verify__tx)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__export)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__generate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__import)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__multi__sig__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__multi__sig__combine__partial__sig)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__show)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__sign)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__sign__kms)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__help__update__alias)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__import)
            opts="-h --alias --json --help <INPUT_STRING> <KEY_SCHEME> [DERIVATION_PATH]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --alias)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__list)
            opts="-s -h --sort-by-alias --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__multi__sig__address)
            opts="-h --threshold --pks --weights --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --threshold)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pks)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --weights)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__multi__sig__combine__partial__sig)
            opts="-h --sigs --pks --weights --threshold --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --sigs)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pks)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --weights)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --threshold)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__show)
            opts="-h --json --help <FILE>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__sign)
            opts="-h --address --data --intent --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --data)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --intent)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__sign__kms)
            opts="-h --data --keyid --intent --base64pk --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --data)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --keyid)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --intent)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --base64pk)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__keytool__update__alias)
            opts="-h --json --help <OLD_ALIAS> [NEW_ALIAS]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move)
            opts="-p -d -h --path --client.config --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --help build coverage disassemble manage-package migrate new test help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --client.config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__build)
            opts="-p -d -h --with-unpublished-dependencies --dump-bytecode-as-base64 --generate-struct-layouts --path --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__coverage)
            opts="-p -d -h --path --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --help summary source bytecode help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__coverage__bytecode)
            opts="-p -d -h --module --path --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --module)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__coverage__help)
            opts="summary source bytecode help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__coverage__help__bytecode)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__coverage__help__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__coverage__help__source)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__coverage__help__summary)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__coverage__source)
            opts="-p -d -h --module --path --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --module)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__coverage__summary)
            opts="-p -d -h --summarize-functions --csv --path --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__disassemble)
            opts="-i -p -d -h --Xdebug --interactive --path --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --help <module_path>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__help)
            opts="build coverage disassemble manage-package migrate new test help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__help__build)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__help__coverage)
            opts="summary source bytecode"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__help__coverage__bytecode)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__help__coverage__source)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__help__coverage__summary)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__help__disassemble)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__help__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__help__manage__package)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__help__migrate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__help__new)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__help__test)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__manage__package)
            opts="-p -d -h --environment --network-id --original-id --latest-id --version-number --path --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --environment)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --network-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --original-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --latest-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --version-number)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__migrate)
            opts="-p -d -h --path --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__new)
            opts="-p -d -h --path --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --help <NAME>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__move__test)
            opts="-i -l -t -s -p -d -h --gas-limit --list --threads --statistics --verbose --coverage --seed --rand-num-iters --path --dev --test --doc --install-dir --force --fetch-deps-only --skip-fetch-latest-git-deps --default-move-flavor --default-move-edition --dependencies-are-root --silence-warnings --warnings-are-errors --json-errors --no-lint --lint --help [filter]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --gas-limit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -i)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --threads)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -t)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --statistics)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -s)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --seed)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --rand-num-iters)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --install-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-flavor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --default-move-edition)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__start)
            opts="-h --network.config --force-regenesis --with-faucet --faucet-amount --fullnode-rpc-port --epoch-duration-ms --no-full-node --local-migration-snapshots --remote-migration-snapshots --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --network.config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --with-faucet)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --faucet-amount)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --fullnode-rpc-port)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --epoch-duration-ms)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --local-migration-snapshots)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --remote-migration-snapshots)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator)
            opts="-y -h --client.config --json --yes --help make-validator-info become-candidate join-committee leave-committee display-metadata update-metadata update-gas-price report-validator serialize-payload-pop display-gas-price-update-raw-txn register-bridge-committee update-bridge-committee-node-url list help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --client.config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__become__candidate)
            opts="-h --gas-budget --json --help <validator-info-path>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__display__gas__price__update__raw__txn)
            opts="-h --sender-address --operation-cap-id --new-gas-price --gas-budget --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --sender-address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --operation-cap-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --new-gas-price)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__display__metadata)
            opts="-h --json --help [validator-address]"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --json)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help)
            opts="make-validator-info become-candidate join-committee leave-committee display-metadata update-metadata update-gas-price report-validator serialize-payload-pop display-gas-price-update-raw-txn register-bridge-committee update-bridge-committee-node-url list help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__become__candidate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__display__gas__price__update__raw__txn)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__display__metadata)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__join__committee)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__leave__committee)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__list)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__make__validator__info)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__register__bridge__committee)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__report__validator)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__serialize__payload__pop)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__bridge__committee__node__url)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__gas__price)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__metadata)
            opts="name description image-url project-url network-address primary-address p2p-address network-pub-key protocol-pub-key authority-pub-key"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__metadata__authority__pub__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__metadata__description)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__metadata__image__url)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__metadata__name)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__metadata__network__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__metadata__network__pub__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__metadata__p2p__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__metadata__primary__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__metadata__project__url)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__help__update__metadata__protocol__pub__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__join__committee)
            opts="-h --gas-budget --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__leave__committee)
            opts="-h --gas-budget --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__list)
            opts="-h --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__make__validator__info)
            opts="-h --json --help <NAME> <DESCRIPTION> <IMAGE_URL> <PROJECT_URL> <HOST_NAME> <GAS_PRICE>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__register__bridge__committee)
            opts="-h --bridge-authority-key-path --bridge-authority-url --print-only --validator-address --gas-budget --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --bridge-authority-key-path)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --bridge-authority-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --validator-address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__report__validator)
            opts="-h --operation-cap-id --undo-report --gas-budget --json --help <reportee-address>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --operation-cap-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --undo-report)
                    COMPREPLY=($(compgen -W "true false" -- "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__serialize__payload__pop)
            opts="-h --account-address --protocol-public-key --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --account-address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --protocol-public-key)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__bridge__committee__node__url)
            opts="-h --bridge-authority-url --print-only --validator-address --gas-budget --json --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --bridge-authority-url)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --validator-address)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__gas__price)
            opts="-h --operation-cap-id --gas-budget --json --help <gas-price>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --operation-cap-id)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata)
            opts="-h --gas-budget --json --help name description image-url project-url network-address primary-address p2p-address network-pub-key protocol-pub-key authority-pub-key help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --gas-budget)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__authority__pub__key)
            opts="-h --json --help <authority-key-path>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__description)
            opts="-h --json --help <DESCRIPTION>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__help)
            opts="name description image-url project-url network-address primary-address p2p-address network-pub-key protocol-pub-key authority-pub-key help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__help__authority__pub__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__help__description)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__help__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__help__image__url)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__help__name)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__help__network__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__help__network__pub__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__help__p2p__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__help__primary__address)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__help__project__url)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__help__protocol__pub__key)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 5 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__image__url)
            opts="-h --json --help <IMAGE_URL>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__name)
            opts="-h --json --help <NAME>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__network__address)
            opts="-h --json --help <NETWORK_ADDRESS>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__network__pub__key)
            opts="-h --json --help <network-key-path>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__p2p__address)
            opts="-h --json --help <P2P_ADDRESS>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__primary__address)
            opts="-h --json --help <PRIMARY_ADDRESS>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__project__url)
            opts="-h --json --help <PROJECT_URL>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        iota__validator__update__metadata__protocol__pub__key)
            opts="-h --json --help <protocol-key-path>"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 4 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _iota -o nosort -o bashdefault -o default iota
else
    complete -F _iota -o bashdefault -o default iota
fi
