#include <lib/std.mi>
#include <lib/pldir.mi>
#include <lib/colormgr.mi>
Global ColorMgr StartupCallback;

#define VIDEO_GUID "{F0816D7B-FFFC-4343-80F2-E8199AA15CC3}"
#define VIS_GUID "{0000000A-000C-0010-FF7B-01014263450C}"
#define ML_GUID "{6B0EDF80-C9A5-11D3-9F26-00C04F39FFC6}"
#define CE_GUID "{00000000-0000-0000-0000-000000000000}"
#define CON_GUID "{7383A6FB-1D01-413B-A99A-7E6F655F4591}"
#define LIR_GUID "{7A8B2D76-9531-43B9-91A1-AC455A7C8242}"
#define DL_GUID "{A3EF47BD-39EB-435A-9FB3-A5D87F6F17A5}"
#define PL_GUID "{45F3F7C1-A6F3-4EE6-A15E-125E92FC3F8D}"
#define JTF_GUID "{5F8D8373-EAA7-4390-B5AB-402E86A5F9DD}"
#define GUID_BLACKLIST "{D6201408-476A-4308-BF1B-7BACA1124B12};{5F8D8373-EAA7-4390-B5AB-402E86A5F9DD};{00000000-0000-0000-0000-000000000000}"
#define DEF_DRAWER_H 119
#define DRAWER_PL_ID 4
#define DRAWER_VID_ID 5
#define DRAWER_VIS_ID 6
#define WIDGET_TAB_ID -666
#define myDelay 10

// Functions used
Function openMainLayoutNow();
Function setDrawer(boolean onOff);
Function openTabNo(int tabNo);
Function openWidgetIDS(String ids);
Function openMini(int miniNo);
Function refreshComponentButtons();
Function setMainFrame(boolean open);
Function setCompStatus(boolean onOff);
Function updateCompStatus();
Function setFrame1();
Function setFrame2(int pos, int h);
Function gotoPrevMini();
Function gotoNextMini();
Function setMiniBG(int mode); //0=normal, 1=tagview
Function setDrawerSizeSave(int h);

// Main Layout
Global Container player;
Global Layout normal;
Global Group xuiGroup;

// Mini Area stuff
Global Group mini_Cover, mini_Video, mini_AVS, mini_SavedPL, mini_TagView;
Global Button but_miniGoto;
Global PopUpMenu popMenu;
Global Boolean mouse_but_miniGoto,  dmr_down;
Global GuiObject gad_Grid;
Global Layer gadgrid1, gadgrid1a, gadgrid1a_overlay, gadgrid2;

// Gui Extras
Global Text plText1, plText2;

// Drawer stuff
Global boolean open_drawer, busyWithDrawer, cuseqbg;
Global ComponentBucket dummyBuck;
Global GuiObject customObj;
Global ToggleButton tog_drawer;
Global Group drawer, mainTabsheet;

// Wasabi:Frame stuff
Global Frame mainFrame, plFrame;
Global boolean mouseDownF1, mouseDownF2, stopResizeRight;
Global Button closeFrame, openFrame;
Global Group area_left, area_right, area_mini, area_right_pl, ocFrame;//, CproTabs;

// Component Handling
Global int delayStartTab, widgetStatus;
Global boolean dontTabCall, skipLoad, busyWithThisFunction, wasTabTrig, delayStart, ml_installed;
Global String closeGUID;
Global int active_tab, tab_openned, delayStartTab;
Global Timer openMainLayout, openDefaultTab, refreshAIOTab, checkVisName, ssWinHol;
Global WindowHolder hold_Other, hold_Pl2, hold_vid, hold_avs, hold_ml;
Global Text visName;
Global GuiObject guihold_Pl2, compGrid;
Global GuiObject visRectBg, CproTabs;
Global Browser xuiBrowser;
Global Group tabbut_vid, tabbut_avs, tabbut_pl;
Global Group tab_library, tab_video, tab_avs, tab_Browser, tab_Playlist, tab_Other, tab_Widget;
Global GuiObject widgetHolder, drawerResize;


System.onScriptLoaded(){
	StartupCallback = new ColorMgr;
	dontTabCall=false;
	delayStart=true;
	
	delayStartTab = 0;
	widgetStatus = 0;
	tab_openned = -1;
	active_tab = getPublicInt("cPro.lastComponentPage", 0);
	ml_installed = stringToInteger(getParam());
	open_drawer=false;
	skipLoad=true;

	// Grab all the objects
	player = getContainer("main");
	normal = player.getLayout("normal");
	xuiGroup = getScriptGroup();
	xuiBrowser = xuiGroup.findObject("cpro.browser");
	mainTabsheet = xuiGroup.findObject("centro.componentsheet");
	mainFrame = xuiGroup.findObject("centro.mainframe");
	plFrame = xuiGroup.findObject("centro.plframe");
	ocFrame = xuiGroup.findObject("centro.componentsheet.opencloseframe");
	CproTabs = xuiGroup.findObject("Cpro.tabs");
	visName = xuiGroup.findObject("centro.visname");
	closeFrame = xuiGroup.findObject("close.rightframe");
	openFrame = xuiGroup.findObject("open.rightframe");
	drawer = xuiGroup.findObject("centro.multidrawer");
	compGrid = xuiGroup.findObject("centro.componentsheet.grid");
	but_miniGoto = xuiGroup.findObject("comp.goto");
	mini_Cover = xuiGroup.findObject("centro.playlist.directory.cov");
	mini_Video = xuiGroup.findObject("centro.playlist.directory.vid");
	mini_AVS = xuiGroup.findObject("centro.playlist.directory.vis");
	mini_SavedPL = xuiGroup.findObject("centro.playlist.directory.spl");
	mini_TagView = xuiGroup.findObject("centro.playlist.directory.tag");
	plText1 = xuiGroup.findObject("centro.playlist.pltext1");
	plText2 = xuiGroup.findObject("centro.playlist.pltext2");
	tabbut_vid = xuiGroup.findObject("centro.video.buttons");
	tabbut_avs = xuiGroup.findObject("centro.visualization.buttons");
	tabbut_pl = xuiGroup.findObject("centro.playlist2.buttons");
	tab_library = xuiGroup.findObject("centro.library");
	tab_video = xuiGroup.findObject("centro.video");
	tab_avs = xuiGroup.findObject("centro.visualization");
	tab_Browser = xuiGroup.findObject("centro.browser");
	tab_Playlist = xuiGroup.findObject("centro.playlist2");
	tab_Other = xuiGroup.findObject("centro.other");
	tab_Widget = xuiGroup.findObject("centro.widget");
	widgetHolder = tab_Widget.findObject("widget.holder");
	tog_drawer = xuiGroup.findObject("tog.drawer");
	area_left = xuiGroup.findObject("centro.components");
	area_right = xuiGroup.findObject("centro.playlist1");
	area_right_pl = xuiGroup.findObject("centro.playlist.component"); 
	area_mini = xuiGroup.findObject("centro.playlist.directory");
	hold_Pl2 = xuiGroup.findObject("centro.windowholder.playlist2");
	guihold_Pl2 = xuiGroup.findObject("centro.windowholder.playlist2");
	hold_vid = xuiGroup.findObject("centro.windowholder.video");
	hold_avs = xuiGroup.findObject("centro.windowholder.visualization");
	hold_ml = xuiGroup.findObject("centro.windowholder.library");
	visRectBg = xuiGroup.findObject("centro.windowholder.visualization.bg");
	dummyBuck = xuiGroup.findObject("widget.loader.mini");
	customObj = xuiGroup.findObject("widget.holder.mini");
	hold_Other = xuiGroup.findObject("centro.windowholder.other");
	gad_Grid = xuiGroup.findObject("centro.mini.grid");
	gadgrid1 = xuiGroup.findObject("centro.bottomleftgrid.1");
	gadgrid1a = xuiGroup.findObject("centro.bottomleftgrid.1a");
	gadgrid1a_overlay = xuiGroup.findObject("centro.bottomleftgrid.1a.overlay");
	gadgrid2 = xuiGroup.findObject("centro.bottomleftgrid.2");
	drawerResize = xuiGroup.findObject("cpro.drawer.resize");



	// Check to see if the new eq background is used
	Map myMap = new Map;
	myMap.loadMap("read.suiframe.png");
	if(myMap.getWidth()>=272) cuseqbg=true;
	else  cuseqbg=false;
	delete myMap;

	// Fix Winamp:Browser for Cpro!
	GuiObject tempbutton;
	Group browserGroup = xuiGroup.findObject("cpro.browser");
	tempbutton = browserGroup.findObject("browser.navigate");
	tempbutton.setXmlParam("text", "Go");
	tempbutton = browserGroup.findObject("search.go");
	tempbutton.setXmlParam("text", "Search");
	Map myMap = new Map;
	myMap.loadMap("browser.fullpng");
	if(myMap.getWidth()>=284){
		tempbutton = browserGroup.findObject("browser.scraper").findObject("browser.dlds.settings");
		tempbutton.setXmlParam("icon_id", "browser.button.settings2");
		tempbutton = browserGroup.findObject("dlds.mode").findObject("scraper.switch");
		tempbutton.setXmlParam("icon_id", "browser.button.scraper2");
		tempbutton = browserGroup.findObject("dlds.mode").findObject("dlds.switch");
		tempbutton.setXmlParam("icon_id", "browser.button.dlds2");
		tempbutton = browserGroup.findObject("scraper.mode").findObject("scraper.switch");
		tempbutton.setXmlParam("icon_id", "browser.button.scraper2");
		tempbutton = browserGroup.findObject("scraper.mode").findObject("dlds.switch");
		tempbutton.setXmlParam("icon_id", "browser.button.dlds2");
	}
	delete myMap;

	// Timers
	openMainLayout = new Timer;
	openDefaultTab = new Timer;
	refreshAIOTab = new Timer;
	checkVisName = new Timer;
	ssWinHol = new Timer;
	openMainLayout.setDelay(myDelay);
	openDefaultTab.setDelay(myDelay);
	refreshAIOTab.setDelay(myDelay);
	checkVisName.setDelay(500);
	ssWinHol.setDelay(66);
	
	//Saved Settings
	openMini(getPublicInt("cPro.lastMini", 0));
	setDrawer(getPublicInt("cPro.draweropened", 0));
	openWidgetIDS(getPublicString("cPro.lastMainWidgetIDS", ""));
	refreshComponentButtons();
}
System.onscriptunloading(){
	delete openMainLayout;
	delete openDefaultTab;
	delete refreshAIOTab;
	delete checkVisName;
	delete ssWinHol;
}

xuiGroup.onSetVisible(boolean onOff){
	int pageNo = getPublicInt("cPro.lastComponentPage", 0);

	if(skipLoad && 	!wasTabTrig){
		//if(pageNo==5) pageNo=0;
		openTabNo(pageNo);
		//debugstring(integertoString(pageNo),9);
		ssWinHol.start();
		skipLoad=false;
	}
	if(mainFrame.getPosition()==0){
		area_right.hide();
	}
	
	if(!onOff){
		if(pageNo==5) openTabNo(0);
	}
}


System.onSetXuiParam(String param, String value) {
	if(strlower(param) == "enable_drawer"){
		setDrawer(stringToInteger(value));
	}
}

System.onOpenUrl(string url){
	openTabNo(4);
	xuiBrowser.sendAction ("openurl", url, 0, 0, 0, 0);
	return 1;
}

System.onGetCancelComponent(String guid, boolean goingvisible){
	//debugString(guid,9);
	//setClipboardText(guid);
	//debug(guid);

	// Check to see if this component is on the blacklist, and if it is, it will open in its own window or just close it...
	for(int i=0;i<10;i++){
		if(getToken(GUID_BLACKLIST, ";", i)== guid){
			return false;
		}
		else if(getToken(GUID_BLACKLIST, ";", i)== ""){
			break;
		}
	}

	//If a component want to open (else = close)
	if(goingvisible){
		
		openMainLayoutNow();

		//Resize player bigger on component activity if size is to small
		if(xuiGroup.getheight()<10){
			normal.resize(getCurAppLeft(),getCurAppTop(),getCurAppWidth(), 300);
		}
		
		if(guid == PL_GUID){ //Playlist
			if(getPublicInt("cPro.lastDrawer", 0)==DRAWER_PL_ID && open_drawer){
				//do nothing
			}
			else if(mainFrame.getPosition()==0){
				hold_Pl2.setXmlParam("autoopen", "0");
				openTabNo(1);
				hold_Pl2.setXmlParam("autoopen", "1");
			}
		}
		else if(guid == ML_GUID){
			if(active_tab!=0){
				hold_ml.setXmlParam("autoopen", "0");
				openTabNo(0);
				hold_ml.setXmlParam("autoopen", "1");
			}
		}
		else if(guid == VIDEO_GUID){
			if(setPublicInt("cPro.lastMini", 0)==1 && area_mini.isVisible()){
				//do nothing
			}
			else if(active_tab!=2){
				hold_vid.setXmlParam("autoopen", "0");
				openTabNo(2);
				hold_vid.setXmlParam("autoopen", "1");
			}
		}
		else if(guid == VIS_GUID){
			if(getPublicInt("cPro.lastMini", 0)==4 && area_mini.isVisible()){
				//do nothing
			}
			else if(getPublicInt("cPro.lastDrawer", 0)==DRAWER_VIS_ID && open_drawer){
				//do nothing
			}
			else if(active_tab!=3){
				hold_avs.setXmlParam("autoopen", "0");
				openTabNo(3);
				hold_avs.setXmlParam("autoopen", "1");
			}
		}
		else{
			if(active_tab!=5){
				openTabNo(5);
			}
			else{
				hold_Other.hide();
				hold_Other.setXMLParam("hold", "");
				hold_Other.show();
				hold_Other.setXMLParam("hold", "@all@");
				//hold_Other.setXMLParam("hold", "guid:"+ guid);
				refreshAIOTab.start();
			}
			return false;
		}
		
		if(delayStart) return true; //dont try to open component because its not visible yet!
		return false;
	}
	else{
		closeGUID = guid;
		
		if(delayStart) return false; //somehow if this is here then if you close with vid tab last open it wont timmy on start
		
		openDefaultTab.start();
		return true;
	}
}

refreshAIOTab.onTimer(){
	refreshAIOTab.stop();
	CproTabs.sendAction("update_tabname", hold_Other.getComponentName(), 5, 0, 0, 0);
}

openDefaultTab.onTimer(){
	openDefaultTab.stop();

	if(closeGUID == PL_GUID){
		if(getPublicInt("cPro.lastDrawer", 0)==DRAWER_PL_ID && open_drawer){
			drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0);
		}
		else if(mainFrame.getPosition()!=0){
			setMainFrame(false);
			return;
		}
	}
	else if(closeGUID == VIDEO_GUID){
		if(getPublicInt("cPro.lastMini", 0)==3){	//video is openned in mini view
			openMini(0);
			return;
		}
	}
	else if(closeGUID == VIS_GUID){
		if(getPublicInt("cPro.lastDrawer", 0)==DRAWER_VIS_ID && open_drawer){
			drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0);
			return;
		}
		else if(getPublicInt("cPro.lastMini", 0)==4){
			openMini(0);
			return;
		}
	}

	if(active_tab==0){
		openTabNo(2);//vid
	}
	else{
		openTabNo(0);//ml
	}
}

openWidgetIDS (String ids)
{
	setPublicString("cPro.lastMainWidgetIDS", ids);
	widgetHolder.setXmlParam("groupid", ids);
}

openTabNo(int tabNo){
	if(delayStart && tabNo!=5){
		delayStartTab=tabNo;
		return;
	}
	else if(delayStart) delayStartTab=tabNo;
	//debugstring("openTabNo()="+integerToString(tabNo),9);

	if(busyWithThisFunction) return;
	busyWithThisFunction=true;
	
	if(skipLoad) wasTabTrig=true;
	active_tab = tabNo;
	
	if(active_tab!=tab_openned){ //check to see if the current tab is already opened...
		tab_library.hide();
		tab_video.hide();
		tab_avs.hide();
		tab_Browser.hide();
		tab_Playlist.hide();
		tab_Other.hide();
		tab_Widget.hide();
	}
	
	tab_openned = tabNo;

	if(tabNo==1){
		if(mainFrame.getPosition()!=0) setMainFrame(false); //if not closed.. close sideview
		if(getPublicInt("cPro.lastDrawer", 0)==DRAWER_PL_ID && open_drawer)	drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0);

		tab_Playlist.show();
	}
	else if(tabNo==2){
		if(getPublicInt("cPro.lastMini", 0)==3){
			openMini(0);
		}
		if(getPublicInt("cPro.lastDrawer", 0)==DRAWER_VID_ID){
			drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0); //close drawer vid
		}
		tab_video.show();
	}
	else if(tabNo==3){
		if(getPublicInt("cPro.lastMini", 0)==4){
			openMini(0);
		}
		if(getPublicInt("cPro.lastDrawer", 0)==DRAWER_VIS_ID){
			drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0); //close drawer vis
		}	
		tab_avs.show();
		checkVisName.start();
	}
	else if(tabNo==4){
		tab_Browser.show();
	}
	else if(tabNo==5){
		tab_Other.show();
		refreshAIOTab.start();
	}
	else if(tabNO==WIDGET_TAB_ID)
	{
		tab_Widget.show();
	}
	else
	{
		tab_library.show();
	}
	
	
	if(tabNo!=-1){
		setPublicInt("cPro.lastComponentPage", tabNo);
	}
	busyWithThisFunction=false;
	updateCompStatus();
	
	if (!dontTabCall)
	{
		if (tabNo==WIDGET_TAB_ID)
		{
			CproTabs.sendAction("select_tab", widgetHolder.getXmlParam("groupid"), tabNo, 0, 0, 0);
		}
		else
		{
			CproTabs.sendAction("select_tab", "", tabNo, 0, 0, 0);	
		}
	}
	else dontTabCall=false;
}

checkVisName.onTimer(){
	checkVisName.stop();
	if(tab_openned==3)	visName.setText(hold_avs.getComponentName());
}

openMainLayoutNow(){
	Boolean isShade = (player.getCurLayout() != normal);
	if(isShade)	openMainLayout.start();
}
openMainLayout.onTimer(){
	player.switchToLayout("normal");
	openMainLayout.stop();
}


area_right.onSetVisible(boolean onOff){
	if(onOff && getPublicInt("cPro.lastComponentPage", 0)==1 && !skipLoad) openTabNo(0); //dont want two playlist hey?
	if(getPublicInt("cPro.lastDrawer", 0)==DRAWER_PL_ID) drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0);
}


area_mini.onResize(int x, int y, int w, int h){
	//Hide/Show the area
	if(w<10){
		area_mini.hide();
		if(getPublicInt("cPro.lastMini", 0)==4){openMini(0);}
	}
	else if(h<40){
		area_mini.hide();
		if(getPublicInt("cPro.lastMini", 0)==4){openMini(0);}
	}
	else area_mini.show();
}


but_miniGoto.onleftClick(){
	popMenu = new PopUpMenu;

	popMenu.addCommand("Album Art", 0, 0, 0);
	popMenu.addCommand("File Info", 1, 0, 0);
	popMenu.addCommand("Stored Playlists", 2, 0, 0);
	popMenu.addSeparator();
	popMenu.addCommand("Video", 3, 0, 0);
	popMenu.addCommand("Visualization", 4, 0, 0);
	popMenu.addSeparator();

	int count = 0;
	for (int x = 0; x < dummyBuck.getNumChildren(); x++) {//**
		GuiObject gr = dummyBuck.enumChildren(x);
		popMenu.addCommand(gr.getXMLparam("name"), 100+x, getPublicInt("cPro.lastMini", 0) == 100+x, 0);
		count++;
	}

	if (count == 0) popMenu.addCommand("No widgets found for this view!", -1, 0, 1);

	popMenu.addSeparator();
	popMenu.addCommand("Widgets Manager", -3, getContainer("widgets.manager").getLayout("normal").isvisible(), 0);

	popMenu.checkCommand(getPublicInt("cPro.lastMini", 0), 1);

	int result = popMenu.popAtXY(clientToScreenX(but_miniGoto.getLeft()), clientToScreenY(but_miniGoto.getTop() + but_miniGoto.getHeight()));

	if(result>=0){
		openMini(result);
	}
	else if(result == -3){
		if (getContainer("widgets.manager").getLayout("normal").isvisible())
		{
			normal.sendAction("widget_manager_hide", "", 0,0,0,0);
		}
		else
		{
			normal.sendAction("widget_manager_show", "", 0,0,0,0);
		}
	}
	
	delete popMenu;
	complete;
}

openMini(int miniNo){
	//Safety check to see if the widgets is still there ;)
	if(miniNo>=100){
		if(dummyBuck.getNumChildren()<miniNo-99) miniNo=0;
	}
	int bg = 0;

	mini_Cover.hide();
	mini_Video.hide();
	mini_AVS.hide();
	mini_SavedPL.hide();
	mini_TagView.hide();
	customObj.hide();
  
	if(miniNo==0){
		mini_Cover.show();
	}
	else if(miniNo==3){
		if(getPublicInt("cPro.lastComponentPage", 0)==2){
			openTabNo(0);
		}
		if(getPublicInt("cPro.lastDrawer", 0)==DRAWER_VID_ID){
				drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0); //close drawer vid
		}
		mini_Video.show();
	}
	else if(miniNo==4){
		if(getPublicInt("cPro.lastComponentPage", 0)==3){
			openTabNo(0);
		}
		if(getPublicInt("cPro.lastDrawer", 0)==DRAWER_VIS_ID){
				drawer.sendAction ("switch_to_drawer", "", 0, 0, 0, 0); //close drawer vis
		}

		mini_AVS.show();
	}
	else if(miniNo==2){
		mini_SavedPL.show();
	}
	else if(miniNo==1){
		drawer.sendAction ("release", "TAG", 0, 0, 0, 0);
		mini_TagView.show();
		bg=1;
	}
	else if(miniNo >= 100) {
		GuiObject gr = dummyBuck.enumChildren(miniNo-100);
		String id = getToken(gr.getXmlParam("userdata"), ";", 0);
		
		// @pjn - add background change here! V1.1
		bg = stringToInteger(getToken(gr.getXmlParam("userdata"), ";", 1));
		
		customObj.setXmlParam("groupid", id);
		customObj.show();
	}
	setPublicInt("cPro.lastMini", miniNo);
	setMiniBG(bg);
}

gotoPrevMini(){ //wheelup
	int pos = getPublicInt("cPro.lastMini", 0);

	if (pos == 0)
	{
		if(dummyBuck.getNumChildren()==0) pos = 2;
		else pos = 99+dummyBuck.getNumChildren();
	}
	else if (pos == 100)
	{
		pos = 2;
	}
	else if(pos>=3 && pos<=4) pos = 2;
	else pos--;

	openMini(pos);
}
gotoNextMini(){ //wheelDown
	int pos = getPublicInt("cPro.lastMini", 0);

	if(pos>=3 && pos<=4) pos = 2;

	if (pos == 2)
	{
		if(dummyBuck.getNumChildren()==0) pos = 0;
		else pos = 100;
	}
	else if (pos == 99+dummyBuck.getNumChildren())
	{
		pos = 0;
	}
	else pos++;

	openMini(pos);
}

normal.onMouseWheelUp(int clicked , int lines){
	if(mouse_but_miniGoto){
		gotoPrevMini();
		complete;
		return 1;
	}
}
normal.onMouseWheelDown(int clicked , int lines){
	if(mouse_but_miniGoto){
		gotoNextMini();
		complete;
		return 1;
	}
}
but_miniGoto.onEnterArea(){
	mouse_but_miniGoto=true;
}
but_miniGoto.onLeaveArea(){
	mouse_but_miniGoto=false;
}
setMiniBG(int mode){
	if(mode==0){
		gadgrid1.show();
		gadgrid1a.hide();
		gadgrid1a_overlay.hide();
		
		gadgrid2.setXmlParam("x", "6");
		gadgrid2.setXmlParam("w", "-6");
	}
	else if(mode==1){
		if(getPublicInt("cPro.transparentsave", 0)){
			gadgrid1.hide();
			gadgrid1a.show();
			gadgrid1a_overlay.show();

			gadgrid2.setXmlParam("x", "247");
			gadgrid2.setXmlParam("w", "-247");
		}
		else{
			gadgrid1.show();
			gadgrid1a.show();
			gadgrid1a_overlay.show();
			
			gadgrid2.setXmlParam("x", "6");
			gadgrid2.setXmlParam("w", "-6");
		}
	}
	else{
		debug("Error: Mini background not found!");
	}
}

refreshComponentButtons(){
	if(getPublicInt("Cpro.One.TabStatus.1", 1)){
			guihold_Pl2.setXmlParam("h", "-27");
			tabbut_pl.show();
	}
	else{
			guihold_Pl2.setXmlParam("h", "-4");
			tabbut_pl.hide();
	}

	if(getPublicInt("Cpro.One.TabStatus.2", 1)){
			hold_vid.setXmlParam("h", "-27");
			tabbut_vid.show();
	}
	else{
			hold_vid.setXmlParam("h", "-4");
			tabbut_vid.hide();
	}
	
	if(getPublicInt("Cpro.One.TabStatus.3", 1)){
			hold_avs.setXmlParam("h", "-27");
			visRectBg.setXmlParam("h", "-27");
			tabbut_avs.show();
	}
	else{
			hold_avs.setXmlParam("h", "-4");
			visRectBg.setXmlParam("h", "-4");
			tabbut_avs.hide();
	}
	updateCompStatus();
}

updateCompStatus(){
	if(tab_openned==0) setCompStatus(false);
	else if(tab_openned==4) setCompStatus(false);
	else if(tab_openned==5) setCompStatus(false);
	else if(tab_openned==WIDGET_TAB_ID) setCompStatus(widgetStatus);
	else setCompStatus(getPublicInt("Cpro.One.TabStatus."+integerToString(tab_openned), 1));
}

setCompStatus(boolean onOff){
	if(cuseqbg){
		if(onOff){
			compGrid.setXmlParam("bottomleft", "player.suiframe.7");
			compGrid.setXmlParam("bottom", "player.suiframe.8");
			compGrid.setXmlParam("bottomright", "player.suiframe.9");
		}
		else{
			compGrid.setXmlParam("bottomleft", "player.suiframe.7.alt");
			compGrid.setXmlParam("bottom", "player.suiframe.8.alt");
			compGrid.setXmlParam("bottomright", "player.suiframe.9.alt");
		}
	}
}


xuiGroup.onAction (String action, String param, int x, int y, int p1, int p2, GuiObject source){
	if (strlower(action) == "switch_to_tab"){ //used by other script to show tab holder & select tab
		if (x == WIDGET_TAB_ID){
			openWidgetIDS(param);
		}
		openTabNo(x);
	}
	else if (strlower(action) == "show_tab"){ //used by the tabs to show tab holder
		dontTabCall=true;
		if (x == WIDGET_TAB_ID)
			openWidgetIDS(param);
		openTabNo(x);
	}
	else if (strlower(action) == "refresh_tab_status"){ //used by the tabs to show tab holder
		refreshComponentButtons();
	}
	else if (strlower(action) == "browser_url"){
		openTabNo(4);
		xuiBrowser.sendAction ("openurl", param, 0, 0, 0, 0);
	}
	else if (strlower(action) == "browser_search"){
		openTabNo(4);
		xuiBrowser.sendAction ("search", param, 0, 0, 0, 0);
	}
	else if (strlower(action) == "drawer_onoff"){
		setDrawer(x);		
	}
	else if (strlower(action) == "switch_to_mini"){
		openMini(x);		
	}
	else if (strlower(action) == "frame_main_setsize"){
		mainFrame.setPosition(x);
	}
	else if (strlower(action) == "frame_mini_setsize"){
		setPublicInt("cpro.e1.frame2", plFrame.getPosition());
		setFrame2(x, area_right_pl.getHeight());
	}
	else if (strlower(action) == "sideview_onoff"){
		if(x==1) setMainFrame(true);
		else setMainFrame(false);
	}
	else if (strlower(action) == "release"){
		if(param=="VIS"){
			if(getPublicInt("cPro.lastMini", 0)==4) openMini(0);
			if(getPublicInt("cPro.lastComponentPage", 0)==3) openTabNo(0);
		}
		else if(param=="PL"){
			if(getPublicInt("cPro.lastComponentPage", 0)==1) openTabNo(0);
			if(mainFrame.getPosition()!=0) setMainFrame(false);
		}
		else if(param=="VID"){
			if(getPublicInt("cPro.lastMini", 0)==3) openMini(0);
			if(getPublicInt("cPro.lastComponentPage", 0)==2) openTabNo(0);
		}
		else if(param=="TAG"){
			if(getPublicInt("cPro.lastMini", 0)==1) openMini(0);
		}
	}
	else if(strlower(action) == "widget_statusbar"){
		widgetStatus=x;
		updateCompStatus();
	}
	else if(strlower(action) == "refresh_drawer_h"){
		setDrawerSizeSave(getPublicInt("ClassicPro.drawer.h", -119));
		setDrawer(getPublicInt("cPro.draweropened", 0));
	}
}

//Main Frame code
xuiGroup.onResize(int x, int y, int w, int h){
	setDrawer(getPublicInt("cPro.draweropened", 0));
	
	if(w<413){
		ocFrame.hide();
		tog_drawer.setXmlParam("x", "-24");
		CproTabs.setXmlParam("w", "-27");

		if(mainFrame.getPosition()!=0){
			mainFrame.setPosition(0);
			setPublicInt("cpro.mainframe.sysclose", 1);
		}
	}
	else{
		tog_drawer.setXmlParam("x", "-48");
		ocFrame.show();
		CproTabs.setXmlParam("w", "-51");
	}
	if(w>=413 && getPublicInt("cpro.mainframe.sysclose", 0)==1){
		setMainFrame(true);
	}

}
area_left.onResize(int x, int y, int w, int h){
	setFrame1();
}

area_right.onResize(int x, int y, int w, int h){
	if(stopResizeRight){
		stopResizeRight = false;
		return;
	}
	stopResizeRight = true;

	if(w<10){
		area_right.hide();

		//Update window size toggle
		closeFrame.hide();
		openFrame.show();
		if(!mouseDownF1) mainFrame.setXmlParam("resizable", "0");
	}
	else{
		area_right.show();

		//Update window size toggle
		closeFrame.show();
		openFrame.hide();
	}
	
	if(w<158 && mainFrame.getPosition()!=0){
		if(mouseDownF1){
			setPublicInt("cpro.mainframe.sysclose", 0);
			mainFrame.setPosition(0);
		}
	}
	setFrame1();

	stopResizeRight = false;
}


mainFrame.onLeftButtonDown(int x, int y){
	setPublicInt("cpro.e1.closeframe.lastpos", mainFrame.getPosition()); //TESTING THIS...CHANGE BACK
	mainFrame.setXmlParam("resizable", "1");
	mouseDownF1=true;
}

mainFrame.onLeftButtonUp(int x, int y){
	mouseDownF1=false;
	if(area_right.getWidth()<10) mainFrame.setXmlParam("resizable", "0");
	else setPublicInt("cpro.e1.closeframe.lastpos", mainFrame.getPosition()); //TESTING THIS...CHANGE BACK
	
	mainFrame.setPosition(mainFrame.getPosition()); //This is done to refresh the hide of the resizer ;)
}

setFrame1(){
	if(mainFrame.getPosition()<180 && mainFrame.getPosition()>=158){
		mainFrame.setPosition(158);
	}
}
closeFrame.onLeftClick(){
	setMainFrame(false);
}
openFrame.onLeftClick(){
	setMainFrame(true);
}

setMainFrame(boolean open){
	setPublicInt("cpro.mainframe.sysclose", 0);
	if(open){
		int pos = getPublicInt("cpro.e1.closeframe.lastpos", 200);
		if(pos<158) pos = 158;
		mainFrame.setXmlParam("resizable", "1");
		mainFrame.setPosition(pos);
	}
	else{
		setPublicInt("cpro.e1.closeframe.lastpos", mainFrame.getPosition());
		mainFrame.setXmlParam("resizable", "0");
		mainFrame.setPosition(0);
	}
	mainFrame.setPosition(mainFrame.getPosition()); //This is done to refresh the hide of the resizer ;)
}


//Mini Frame code
plFrame.onLeftButtonDown(int x, int y){
	mouseDownF2=true;
}
plFrame.onLeftButtonUp(int x, int y){
	mouseDownF2=false;
	setPublicInt("cpro.e1.frame2", plFrame.getPosition());

}
area_right_pl.onResize(int x, int y, int w, int h){
	if(mouseDownF2)	setPublicInt("cpro.e1.frame2", plFrame.getPosition());
	setFrame2(getPublicInt("cpro.e1.frame2", plFrame.getPosition()), h);
}
setFrame2(int pos, int h){
	int output = pos;

	//Magnet-clip the framedivider
	if(pos<40){
		output=0;
	}
	else{
		output=xuiGroup.getHeight()-48;
	}
	
	if(output<40) output=0;
	if(output>getPublicInt("cpro.e1.frame2", plFrame.getPosition())) output=getPublicInt("cpro.e1.frame2", plFrame.getPosition());
	
	if(output!=plFrame.getPosition()) plFrame.setPosition(output);
}

plText1.onLeftButtonDblClk(int x, int y){
	PlEdit.showCurrentlyPlayingTrack ();
}
plText2.onLeftButtonDblClk(int x, int y){
	PlEdit.showCurrentlyPlayingTrack ();
}

ssWinHol.onTimer(){
	delayStart=false;
	ssWinHol.stop();
	openTabNo(delayStartTab);
}









setDrawer(boolean onOff){
	busyWithDrawer=true;
	boolean dontSave = false;

	//if(xuiGroup.getHeight()<DEF_DRAWER_H+90){ //0 = first load... not drawn yet
	if(xuiGroup.getHeight()<drawer.getHeight()+90){ //0 = first load... not drawn yet
		onOff=false;
		dontSave=true;
	}
	if(onOff){
		drawer.show();
		drawerResize.show();
		//mainTabsheet.setXmlParam("h", integerToString(DEF_DRAWER_H*-1-4));
		setDrawerSizeSave(getPublicInt("ClassicPro.drawer.h", -119));
		tog_drawer.setXmlParam("tooltip", "Close drawer");
		open_drawer=true;
	}
	else{
		drawer.hide();
		drawerResize.hide();
		mainTabsheet.setXmlParam("h", "0");
		open_drawer=false;
		tog_drawer.setXmlParam("tooltip", "Open drawer");
	}
	tog_drawer.setActivated(open_drawer);

	if(!dontSave){
		setPublicInt("cPro.draweropened", open_drawer);
	}
	busyWithDrawer=false;
}
tog_drawer.onToggle(Boolean onoff){
	setDrawer(onoff);
}

drawer.onSetVisible(boolean onOff){
	if(!busyWithDrawer) setDrawer(getPublicInt("cPro.draweropened", 0));
}





drawerResize.onLeftButtonDown(int x, int y){
	dmr_down=true;
}
drawerResize.onLeftButtonUp(int x, int y){
	dmr_down=false;
}
drawerResize.onMouseMove(int x, int y){
	if(dmr_down){
		int t = y-xuiGroup.getHeight()-xuiGroup.getTop();
		if(t<-xuiGroup.getHeight()+120) t=-xuiGroup.getHeight()+120;
		
		if(getPublicInt("cPro.lastDrawer", 0)==0) return;
		setDrawerSizeSave(t);
	}
}

setDrawerSizeSave(int h){

	//these changes will be saved
	if(h>-119) h=-119;
	setPublicInt("ClassicPro.drawer.h", h);
	

	//these changes will not be saved
	if(getPublicInt("cPro.lastDrawer", 0)==0) h=-119;
	//if(getPublicInt("cPro.lastDrawer", 0)==0)  h= xuiGroup.getHeight()-xuiGroup.getTop();

	mainTabsheet.setXmlParam("h", integerToString(h-4));
	drawer.setXmlParam("y", integerToString(h));
	drawer.setXmlParam("h", integerToString(-h));
	drawerResize.setXmlParam("y", integerToString(h-4));
}

area_mini.onAction (String action, String param, Int x, int y, int p1, int p2, GuiObject source)
{
	if (strlower(action) == "show_widget")
	{
		for (int i = 0; i < dummyBuck.getNumChildren(); i++)
		{
			GuiObject d = dummyBuck.enumChildren(i);
			if (getToken(d.getXmlParam("userdata"), ";",0) == param)
			{
				normal.getContainer().switchToLayout("normal");

				boolean resize = false;
				int w = normal.getWidth();
				int h = normal.getHeight();

				if (normal.getHeight() < 238)
				{
					resize = true;
					h = 388;
				}
				if (normal.getWidth() < 427)
				{
					resize = true;
					w = 427;
				}
				if (resize)
				{
					setPublicInt("cPro.h",h);
					setPublicInt("cPro.w",w);
					normal.resize(normal.getLeft(), normal.getTop(), w, h);	
				}
				
				setMainFrame(true);

				if (plFrame.getPosition() < 40)
				{
					setPublicInt("cpro.e1.frame2", 160);
					setFrame2(160,0);
				}

				openMini(i+100);	
			}
		}	
	}
}


Function cProLoaded();

/* Notify Widget Manager, what cool widgets has been installed */ 
StartupCallback.onLoaded()
{
	cProLoaded();
}

/** somehow winamp creates PVC calls when i do this stuff above */
cProLoaded ()
{
	int widgetPlace = normal.onAction("widget_manager_register", "Side Area", 0,0,0,0,area_mini); // TODO Translate
	//normal.sendAction("widget_manager_register", "Side Area", 0,0,0,0); 
	for (int i = 0; i < dummyBuck.getNumChildren(); i++)
	{
		GuiObject d = dummyBuck.enumChildren(i);
		normal.onAction("widget_manager_check", getToken(d.getXmlParam("userdata"), ";", 0), widgetPlace,0,0,0,d);
		//normal.sendAction("widget_manager_check", getToken(d.getXmlParam("userdata"), ";", 0), 0,0,0,0);
	}
	widgetPlace = normal.onAction("widget_manager_done", "", widgetPlace,0,0,0,area_mini);
	//normal.sendAction("widget_manager_done", "", 0,0,0,0);
}