 #include <lib/std.mi>
//#include <lib/fileio.mi>
#include <lib/colormgr.mi>
Global ColorMgr StartupCallback;
#include <lib/config.mi>

// this is the page that maps its items to the windows menu (aka View), you can add attribs or more pages (submenus)
#define CUSTOM_WINDOWSMENU_ITEMS "{6559CA61-7EB2-4415-A8A9-A2AEEF762B7F}"


Function openDrawer(int drawerNo);
Function gotoPrevDrawer();
Function gotoNextDrawer();
//Function setDrawerBG(int mode); //0=normal, 1=tagview, 2=neweq

/* CPro Widget */
Class Group CProWidget;
Member boolean CProWidget.scrollSkip;
Member boolean CProWidget.disabled;
//Member int CProWidget.custombg;
Member boolean CProWidget.hideVis;
Member boolean CProWidget.hidePL;
Member boolean CProWidget.addSep;

#define userWidgetOffset 100
Global int numUserWidgets = 0;
Global int numInternalWidgets = 0;

Global Layout myLayout, suiLayout;
Global Group myGroup;
Global CProWidget drawer_equalizer, drawer_pl, drawer_vid, drawer_savedpl, drawer_tagviewer, drawer_avs, drawer_ct, drawer_skinchooser;
Global PopUpMenu popMenu;//, widgetmenu;
Global Button but_drawerGoto;
Global GuiObject cpro_sui, gad_Grid, gad_GridEQ;
Global Layer ct_fakeLayer, tempfix;
Global Boolean mouse_but_drawerGoto, cuseqbg, transparentSave;
//Global XmlDoc myDoc;
//Global Layer gadgrid1, gadgrid1a, gadgrid1a_overlay, gadgrid2, gadgrid3;
//Global Layer l_statusbar;

Global ComponentBucket dummyBuck;
Global GuiObject customObj;
Global List internalWidgets;

Global ConfigItem custom_windows_page;
Global ConfigAttribute sui_eq_attrib;



System.onScriptLoaded() {
	StartupCallback = new ColorMgr;
	/*myDoc = new XmlDoc;
	String fullpath = getParam()+"ClassicPro.xml";
	myDoc.load(fullpath);
	transparentSave=false;
	setPublicInt("cpro2.transparentsave", 0);
 	if(myDoc.exists()){
		myDoc.parser_addCallback("ClassicPro");
		myDoc.parser_start();
		myDoc.parser_destroy();
	}
	delete myDoc;*/



	/** Load Window menu */
	custom_windows_page = Config.getItem(CUSTOM_WINDOWSMENU_ITEMS);
	sui_eq_attrib = custom_windows_page.newAttribute("Equalizer\tAlt+G", "0");



	internalWidgets = new List;

	myLayout = getContainer("main").getLayout("normal");

	myGroup = getScriptGroup();
	but_drawerGoto = myGroup.findObject("drawer.menulist");
	ct_fakeLayer = myGroup.findObject("drawer.ct.fakelayer"); //used to detect if skin have colorthemes
	tempfix = myGroup.findObject("tempfix");
	//gadgrid1 = myGroup.findObject("centro.blf.1");
	//gadgrid1a = myGroup.findObject("centro.blf.1a");
	//gadgrid1a_overlay = myGroup.findObject("centro.blf.1a.overlay");
	//gadgrid2 = myGroup.findObject("centro.blf.2");
	//gadgrid3 = myGroup.findObject("centro.blf.3");
	//l_statusbar = myGroup.findObject("cpro.drawer.status");

	/*	ClassicPro Components */
	drawer_equalizer = myGroup.findObject("drawer.equalizer");
	drawer_tagviewer = myGroup.findObject("drawer.tagviewer");
	drawer_savedpl = myGroup.findObject("drawer.savedpl");
	drawer_ct = myGroup.findObject("drawer.colortheme");
	drawer_ct.disabled = ct_fakeLayer.isInvalid();
	drawer_ct.addSep = true;
	internalWidgets.addItem(drawer_equalizer);
	internalWidgets.addItem(drawer_tagviewer);
	internalWidgets.addItem(drawer_savedpl);
	internalWidgets.addItem(drawer_ct);
	/*	end	*/
	
	
	
	/*	Winamp Components */
	drawer_pl = myGroup.findObject("drawer.playlist");
	drawer_pl.scrollSkip = true;
	drawer_vid = myGroup.findObject("drawer.video");
	drawer_vid.scrollSkip = true;
	drawer_avs = myGroup.findObject("drawer.avs");
	drawer_avs.scrollSkip = true;
	drawer_avs.hideVis = true;
	internalWidgets.addItem(drawer_pl);
	internalWidgets.addItem(drawer_vid);
	internalWidgets.addItem(drawer_avs);
	/*	end	*/




	/*drawer_skinchooser = myGroup.findObject("drawer.skinchooser");
	internalWidgets.addItem(drawer_skinchooser);*/

	numInternalWidgets = internalWidgets.getNumItems();

	//gad_Grid = myGroup.findObject("centro.gadget.grid");
	gad_GridEQ = myGroup.findObject("centro.gadget.grid.eq");
	
	suiLayout = myGroup.getParentLayout();
	cpro_sui = suiLayout.findObject("cpro.sui");

	dummyBuck = myGroup.findObject("widget.loader");
	customObj = myGroup.findObject("widget.holder");

	numUserWidgets = dummyBuck.getNumChildren();

	/*Map myMap = new Map;
	myMap.loadMap("read.suiframe.png");
	if(myMap.getWidth()>=272) cuseqbg=true;
	else  cuseqbg=false;
	delete myMap;*/

	//Saved Settings
	openDrawer(getPublicInt("cpro2.lastDrawer", 1));
	if(getPublicInt("cpro2.lastDrawer", 0)!=0) sui_eq_attrib.setData("0");
}
/*
myDoc.parser_onCallback (String xmlpath, String xmltag, list paramname, list paramvalue){
	String busyWith ="";
	for(int i=0; i<paramname.getNumItems(); i++){
		if(paramname.enumItem(i)=="version"){
			transparentSave=true;
			setPublicInt("cpro2.transparentsave", 1);
		}
	}
}
*/


// ADD EQUILIZER SHORTCUT AND OPTION IN MAIN MENU!
sui_eq_attrib.onDataChanged(){
	if(sui_eq_attrib.getData()=="1" && !drawer_equalizer.isVisible())
	{
		cpro_sui.sendAction ("drawer_onoff", "", 1, 0, 0, 0);
		openDrawer(0);
	}
	else if(sui_eq_attrib.getData()=="0" && drawer_equalizer.isVisible()){
		//openDrawer(1);
		cpro_sui.sendAction ("drawer_onoff", "", 0, 0, 0, 0);
	}
}
drawer_equalizer.onSetVisible(boolean onOff){
	if(onOff != stringToInteger(sui_eq_attrib.getData())) sui_eq_attrib.setData(integerToString(onOff));
}
System.onKeyDown(String key) {
	if (key == "alt+g")
	{
		if (sui_eq_attrib.getData() == "0") sui_eq_attrib.setData("1");
		else sui_eq_attrib.setData("0");
		complete;
	}
}


but_drawerGoto.onleftClick(){
	popMenu = new PopUpMenu;

	// Faster to load it once!
	int cur = getPublicInt("cpro2.lastDrawer", 1);

	for ( int i = 0; i < numInternalWidgets; i++ )
	{
		CProWidget gr = internalWidgets.enumItem(i);
		popMenu.addCommand(gr.getXMLparam("name"), i, cur == i, gr.disabled);
		if(gr.addSep) popMenu.addSeparator();
	}
	popMenu.addSeparator();
	//debug("abc");
	//widgetmenu = new PopUpMenu;

	for (int x=0; x < numUserWidgets; x++) {
		GuiObject gr = dummyBuck.enumChildren(x);
		//widgetmenu.addCommand(gr.getXMLparam("name"), userWidgetOffset+x, cur == userWidgetOffset+x, 0);
		popMenu.addCommand(gr.getXMLparam("name"), userWidgetOffset+x, cur == userWidgetOffset+x, 0);
	}
	//debug("abc1");

	if (numUserWidgets == 0) popMenu.addCommand("No widgets found for this view!", -1, 0, 1);
	//popMenu.addSubMenu(widgetmenu, "Widgets");

	popMenu.addSeparator();
	popMenu.addCommand("Widgets Manager", -3, getContainer("widgets.manager").getLayout("normal").isvisible(), 0);
	popMenu.addSeparator();
	popMenu.addCommand("Close drawer", -2, 0, 0);//** Item code changed to -2 to support widgets.

	//popMenu.checkCommand(getPublicInt("cPro.lastDrawer", 0), 1);
	//debug("abc2");

	int result = popMenu.popAtXY(clientToScreenX(but_drawerGoto.getLeft()), clientToScreenY(but_drawerGoto.getTop() + but_drawerGoto.getHeight()));

	if(result>=0) openDrawer(result);
	else if(result == -2){
		setPublicInt("cpro2.draweropened", 0);
		myGroup.hide();
	}
	else if(result == -3){
		if (getContainer("widgets.manager").getLayout("normal").isvisible())
		{
			myLayout.sendAction("widget_manager_hide", "", 0,0,0,0);
		}
		else
		{
			myLayout.sendAction("widget_manager_show", "", 0,0,0,0);
		}
	}

	delete popMenu;
	//delete widgetmenu;
	complete;
}

openDrawer(int drawerNo){

	//Safety check to see if the widgets is still there ;)
	if(drawerNo>=userWidgetOffset){
		if (drawerNo - userWidgetOffset > dummyBuck.getNumChildren()-1)
		{
			drawerNo=0;
		}
		
	}
	
	for ( int i = 0; i < numInternalWidgets; i++ )
	{
		CProWidget gr = internalWidgets.enumItem(i);
		gr.hide();
	}
	customObj.hide();

	// We have to show an Internal Widget
	if (drawerNo < userWidgetOffset)
	{
		CProWidget gr = internalWidgets.enumItem(drawerNo);

		if (gr.disabled == true)
		{
		debug("gr.disabled");
		drawerNo = 0;
			gr = internalWidgets.enumItem(drawerNo); // Load Default Widget
		}

		//setDrawerBG(gr.custombg);

		if(gr.getXMLparam("name")=="Visualization"){
			cpro_sui.sendAction ("release", "VIS", 0, 0, 0, 0);
			setPublicString("cpro2.defaultview.vis", "DRAWER");
		}
		else if (gr.getXMLparam("name")=="Playlist"){
			cpro_sui.sendAction ("release", "PL", 0, 0, 0, 0);
		}
		else if (gr.getXMLparam("name")=="Video"){
			cpro_sui.sendAction ("release", "VID", 0, 0, 0, 0);
			setPublicString("cpro2.defaultview.vid", "DRAWER");
		}
		else if (gr.getXMLparam("name")=="File Info"){ //check if this works right with langauges!!!
			cpro_sui.sendAction ("release", "TAG", 0, 0, 0, 0);
		}

		gr.show();
	}
	else
	{
		GuiObject gr = dummyBuck.enumChildren(drawerNo-userWidgetOffset);
		String id = getToken(gr.getXMLparam("userdata"), ";", 0);
		customObj.setXmlParam("groupid", id);
		//setDrawerBG(stringToInteger(getToken(gr.getXMLparam("userdata"), ";", 2)));
		customObj.show();
	}

	setPublicInt("cpro2.lastDrawer", drawerNo);
	cpro_sui.sendAction ("refresh_drawer_h", "", 0, 0, 0, 0);
	
}

myGroup.onAction (String action, String param, int x, int y, int p1, int p2, GuiObject source){
	if (strlower(action) == "switch_to_drawer") openDrawer(x);
	else if (strlower(action) == "release"){
		//debug("My Wereld");
		if(param=="TAG") if(getPublicInt("cpro2.lastDrawer", 1)==1) openDrawer(0);
	}
	else if (strlower(action) == "show_widget")
	{
		for (int i = 0; i < dummyBuck.getNumChildren(); i++)
		{
			GuiObject d = dummyBuck.enumChildren(i);
			if (getToken(d.getXmlParam("userdata"), ";",0) == param)
			{
				myLayout.getContainer().switchToLayout("normal");
				if (myLayout.getHeight() < 360)
				{
					setPublicInt("cpro2.h",388);
					myLayout.resize(myLayout.getLeft(), myLayout.getTop(), myLayout.getWidth(), 388);
				}
				
				setPublicInt("cpro2.draweropened", 1);
				openDrawer(i+userWidgetOffset);
				myGroup.show();
				return;
			}
		}		
	}
	
}

but_drawerGoto.onEnterArea(){
	mouse_but_drawerGoto=true;
}
but_drawerGoto.onLeaveArea(){
	mouse_but_drawerGoto=false;
}

suiLayout.onMouseWheelUp(int clicked , int lines){
	if(mouse_but_drawerGoto){
		gotoPrevDrawer();
		complete;
		return 1;
	}
}
suiLayout.onMouseWheelDown(int clicked , int lines){
	if(mouse_but_drawerGoto){
		gotoNextDrawer();
		complete;
		return 1;
	}
}

gotoPrevDrawer(){ //wheelup
	int pos = getPublicInt("cpro2.lastDrawer", 1);

	if (pos == userWidgetOffset){
		pos = numInternalWidgets-1;
	}
	else if(pos == 0){
		if(numUserWidgets==0) pos = numInternalWidgets-1;
		else pos = userWidgetOffset + numUserWidgets-1;
	}
	else pos--;

	if (pos < userWidgetOffset){
		CProWidget gr = internalWidgets.enumItem(pos);
		if (gr.scrollSkip || gr.disabled){
			setPublicInt("cpro2.lastDrawer", pos);
			gotoPrevDrawer();
			return;
		}
	}
	openDrawer(pos);
}

gotoNextDrawer(){ //wheelDown
	int pos = getPublicInt("cpro2.lastDrawer", 1);

	if(pos == userWidgetOffset + numUserWidgets -1){
		pos = 0;
	}
	else if(pos == numInternalWidgets-1){
		if(numUserWidgets==0) pos = 0;
		else pos = userWidgetOffset;
	}
	else pos++;

	if (pos < userWidgetOffset){
		CProWidget gr = internalWidgets.enumItem(pos);
		if (gr.scrollSkip || gr.disabled)
		{
			setPublicInt("cpro2.lastDrawer", pos);
			gotoNextDrawer();
			return;
		}
	}
	openDrawer(pos);
}
/*
setDrawerBG(int mode){
	if(mode==0){
		l_statusbar.setXmlParam("relatx", "0");
		l_statusbar.setXmlParam("relatw", "1");
		l_statusbar.setXmlParam("x", "0");
		l_statusbar.setXmlParam("w", "0");
	}
	else{
		l_statusbar.setXmlParam("relatx", "1");
		l_statusbar.setXmlParam("relatw", "0");
		l_statusbar.setXmlParam("x", integerToString(-mode));
		l_statusbar.setXmlParam("w", integerToString(mode));
	}
}
*/

Function cProLoaded();

/* Notify Widget Manager, what cool widgets has been installed */ 
StartupCallback.onLoaded(){
	cProLoaded();
}

/** somehow winamp creates PVC calls when i do this stuff above */
cProLoaded (){
	int widgetPlace = myLayout.onAction("widget_manager_register", "Drawer Area", 0,0,0,0,myGroup); // TODO Translate
	for (int i = 0; i < dummyBuck.getNumChildren(); i++)
	{
		GuiObject d = dummyBuck.enumChildren(i);
		myLayout.onAction("widget_manager_check", getToken(d.getXmlParam("userdata"), ";", 0), widgetPlace,0,0,0,d);
	}
	widgetPlace = myLayout.onAction("widget_manager_done", "", widgetPlace,0,0,0,myGroup);
}